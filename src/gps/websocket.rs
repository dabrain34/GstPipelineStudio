// websocket.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

//! WebSocket support for remote GStreamer pipeline introspection.
//!
//! This module enables GstPipelineStudio to connect to running GStreamer pipelines
//! via WebSocket and visualize their graph structure in real-time. It requires
//! GStreamer Rust tracers from [gst-plugins-rs](https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs).
//!
//! # Listen Mode (GPS as Server)
//!
//! GPS starts a WebSocket server and waits for a tracer to connect.
//! Use this with the `pipeline-snapshot` tracer.
//!
//! 1. In GPS: **Menu → Remote Pipeline → Listen...**
//! 2. Enter the WebSocket address (e.g., `ws://localhost:8080`)
//! 3. GPS displays a waiting dialog
//! 4. Run your GStreamer pipeline with the tracer:
//!
//! ```bash
//! GST_TRACERS="pipeline-snapshot(dots-viewer-ws-url=ws://localhost:8080)" \
//!   gst-launch-1.0 videotestsrc ! autovideosink
//! ```
//!
//! The pipeline graph will appear in GPS once the tracer connects.
//!
//! **Note:** Only the first pipeline is used when multiple pipelines are present
//! in the response.
//!
//! # Installing GStreamer Rust Tracers
//!
//! The tracers are part of `gst-plugins-rs`. Check if they're available:
//!
//! ```bash
//! gst-inspect-1.0 | grep pipeline-snapshot
//! ```
//!
//! If not installed, build from source:
//!
//! ```bash
//! git clone https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs.git
//! cd gst-plugins-rs
//! cargo build --release -p gst-plugin-tracers
//! export GST_PLUGIN_PATH=$PWD/target/release:$GST_PLUGIN_PATH
//! ```

use crate::app::GPSAppWeak;
use crate::logger;
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use thiserror::Error;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{accept_with_config, Message};
use url::Url;

// ============================================================================
// Constants
// ============================================================================

/// Maximum allowed WebSocket message size (1 MB) to prevent DoS via memory exhaustion
const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Overall operation timeout (60 seconds) to prevent indefinite blocking
const OPERATION_TIMEOUT: Duration = Duration::from_secs(60);

/// Read timeout for WebSocket operations (100ms for responsive cancellation)
const READ_TIMEOUT: Duration = Duration::from_millis(100);

/// Maximum number of non-Hello messages to ignore before treating as protocol violation
const MAX_IGNORED_MESSAGES: usize = 10;

// ============================================================================
// Error types
// ============================================================================

/// Errors that can occur during WebSocket operations.
#[derive(Debug, Error)]
pub enum WebSocketError {
    /// Operation was cancelled by user
    #[error("Operation cancelled")]
    Cancelled,

    /// Invalid WebSocket URL format
    #[error("Invalid WebSocket URL '{0}': {1}")]
    InvalidUrl(String, String),

    /// Connection error
    #[error("Connection error: {0}")]
    Connection(String),

    /// Protocol error
    #[error("Protocol error: {0}")]
    Protocol(String),
}

// ============================================================================
// URL parsing
// ============================================================================

/// Parsed WebSocket address with host and port.
#[derive(Debug, Clone)]
pub struct WsAddress {
    pub host: String,
    pub port: u16,
}

impl WsAddress {
    /// Parse a WebSocket URL into host and port.
    /// Only supports ws:// scheme (TLS not implemented). Returns error for invalid URLs.
    pub fn parse(ws_addr: &str) -> Result<Self, WebSocketError> {
        let url = Url::parse(ws_addr)
            .map_err(|e| WebSocketError::InvalidUrl(ws_addr.to_string(), e.to_string()))?;

        // Only ws:// is supported - TLS would require certificate management
        if url.scheme() != "ws" {
            return Err(WebSocketError::InvalidUrl(
                ws_addr.to_string(),
                format!(
                    "Only 'ws://' scheme is supported (TLS not implemented), got '{}'",
                    url.scheme()
                ),
            ));
        }

        let host = url
            .host_str()
            .ok_or_else(|| {
                WebSocketError::InvalidUrl(ws_addr.to_string(), "missing host".to_string())
            })?
            .to_string();

        // port_or_known_default() returns the explicit port or the scheme's default (80 for ws://)
        let port = url.port_or_known_default().unwrap_or(80);

        Ok(Self { host, port })
    }

    /// Returns the bind address for server mode.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

// ============================================================================
// Protocol structures
// ============================================================================

/// Request for Snapshot protocol.
#[derive(Debug, Serialize)]
pub(crate) struct SnapshotRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    #[serde(rename = "type")]
    pub(crate) msg_type: String,
}

/// Generic message to check type field
#[derive(Debug, Deserialize)]
pub(crate) struct TypedMessage {
    #[serde(rename = "type")]
    pub(crate) msg_type: String,
}

/// Response from Snapshot protocol
#[derive(Debug, Deserialize)]
pub(crate) struct SnapshotResponse {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub(crate) msg_type: String,
    pub(crate) pipelines: Vec<SnapshotPipeline>,
}

/// Pipeline info from Snapshot response
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct SnapshotPipeline {
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) id: Option<String>,
    #[serde(default)]
    pub(crate) dot: Option<String>,
}

// ============================================================================
// Server mode (for pipeline-snapshot tracer)
// ============================================================================

/// Cancellation handle for the WebSocket server.
/// Call `cancel()` to stop the server from another thread.
#[derive(Clone)]
pub struct ServerHandle {
    cancelled: Arc<AtomicBool>,
}

impl ServerHandle {
    pub(crate) fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Cancel the server. This will cause `run_server` to return with a cancellation error.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Run GPS as a WebSocket server for pipeline-snapshot tracer.
/// The tracer connects to GPS, sends Hello, and GPS requests Snapshot.
/// This runs in a separate thread to avoid blocking the GTK main loop.
/// Returns a ServerHandle that can be used to cancel the server immediately
/// (the handle is valid before the async task completes).
pub fn run_server(
    ws_addr: &str,
    app_weak: GPSAppWeak,
    on_complete: impl FnOnce(Result<(), WebSocketError>) + 'static,
) -> Result<ServerHandle, WebSocketError> {
    // Parse and validate address before starting
    let ws_address = WsAddress::parse(ws_addr)?;
    let bind_addr = ws_address.bind_addr();

    GPS_INFO!("Starting WebSocket server on {}", bind_addr);

    // Create handle before spawning thread so it can be cancelled immediately
    let handle = ServerHandle::new();
    let handle_clone = handle.clone();

    // Spawn blocking server in a thread
    let (sender, receiver) = async_channel::bounded::<Result<String, WebSocketError>>(1);

    thread::spawn(move || {
        let result = run_server_blocking(&bind_addr, &handle_clone);
        let _ = sender.send_blocking(result);
    });

    // Spawn async task to wait for result
    let ctx = glib::MainContext::default();
    ctx.spawn_local(async move {
        let result = receiver.recv().await;

        match result {
            Ok(Ok(dot_content)) => {
                // Load DOT content in the main thread
                if let Some(app) = app_weak.upgrade() {
                    app.load_dot_content(&dot_content);
                }
                on_complete(Ok(()));
            }
            Ok(Err(e)) => {
                on_complete(Err(e));
            }
            Err(e) => {
                on_complete(Err(WebSocketError::Connection(format!(
                    "Channel error: {}",
                    e
                ))));
            }
        }
    });

    Ok(handle)
}

/// Helper to close websocket with logging.
/// Flushes pending writes before closing.
fn close_websocket(websocket: &mut tungstenite::WebSocket<std::net::TcpStream>) {
    // Flush any pending writes before closing
    if let Err(e) = websocket.flush() {
        GPS_DEBUG!("WebSocket flush warning: {}", e);
    }
    if let Err(e) = websocket.close(None) {
        GPS_DEBUG!("WebSocket close warning: {}", e);
    }
}

/// Blocking server implementation that runs in a separate thread.
/// Uses non-blocking mode with polling to support cancellation.
pub(crate) fn run_server_blocking(
    bind_addr: &str,
    handle: &ServerHandle,
) -> Result<String, WebSocketError> {
    let start_time = Instant::now();

    let listener = TcpListener::bind(bind_addr).map_err(|e| {
        WebSocketError::Connection(format!("Failed to bind to {}: {}", bind_addr, e))
    })?;

    // Security warning for non-localhost bindings
    let is_localhost = bind_addr.starts_with("localhost:")
        || bind_addr.starts_with("127.")
        || bind_addr.starts_with("[::1]:");
    if !is_localhost {
        GPS_WARN!(
            "WebSocket server binding to '{}' - this may expose the server to the network",
            bind_addr
        );
    }

    // Set non-blocking mode to allow cancellation checks
    listener
        .set_nonblocking(true)
        .map_err(|e| WebSocketError::Connection(format!("Failed to set non-blocking: {}", e)))?;

    GPS_INFO!(
        "Listening on {} (waiting for tracer to connect...)",
        bind_addr
    );

    // Accept one connection with cancellation support
    let stream = loop {
        if handle.is_cancelled() {
            return Err(WebSocketError::Cancelled);
        }
        if start_time.elapsed() > OPERATION_TIMEOUT {
            return Err(WebSocketError::Connection(
                "Operation timed out".to_string(),
            ));
        }

        match listener.accept() {
            Ok((stream, peer_addr)) => {
                GPS_INFO!("Client connected from {}", peer_addr);
                break stream;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No connection yet, sleep briefly and retry
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => {
                return Err(WebSocketError::Connection(format!(
                    "Failed to accept connection: {}",
                    e
                )));
            }
        }
    };

    GPS_INFO!("Connection accepted, upgrading to WebSocket...");

    // Set socket to blocking mode for WebSocket operations with timeout
    stream
        .set_nonblocking(false)
        .map_err(|e| WebSocketError::Connection(format!("Failed to set blocking mode: {}", e)))?;
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .map_err(|e| WebSocketError::Connection(format!("Failed to set read timeout: {}", e)))?;

    // Upgrade to WebSocket with message size limits enforced at the protocol layer
    // This prevents OOM from oversized frames before they are fully allocated
    let ws_config = WebSocketConfig {
        max_message_size: Some(MAX_MESSAGE_SIZE),
        max_frame_size: Some(MAX_MESSAGE_SIZE),
        ..Default::default()
    };
    let mut websocket = accept_with_config(stream, Some(ws_config))
        .map_err(|e| WebSocketError::Protocol(format!("WebSocket handshake failed: {}", e)))?;

    // Wait for Hello message from tracer
    let mut ignored_message_count = 0;
    loop {
        if handle.is_cancelled() {
            close_websocket(&mut websocket);
            return Err(WebSocketError::Cancelled);
        }
        if start_time.elapsed() > OPERATION_TIMEOUT {
            close_websocket(&mut websocket);
            return Err(WebSocketError::Connection(
                "Operation timed out".to_string(),
            ));
        }

        match websocket.read() {
            Ok(Message::Text(text)) => {
                // Check message size to prevent DoS
                if text.len() > MAX_MESSAGE_SIZE {
                    close_websocket(&mut websocket);
                    return Err(WebSocketError::Protocol(format!(
                        "Message too large: {} bytes (max: {} bytes)",
                        text.len(),
                        MAX_MESSAGE_SIZE
                    )));
                }

                GPS_DEBUG!("Received: {}", truncate_for_log(&text, 200));

                if let Ok(typed_msg) = serde_json::from_str::<TypedMessage>(&text) {
                    if typed_msg.msg_type == "Hello" {
                        GPS_INFO!("Received Hello from tracer");
                        break;
                    }
                    // Detect protocol violation: SnapshotResponse before Hello
                    if typed_msg.msg_type == "SnapshotResponse" {
                        close_websocket(&mut websocket);
                        return Err(WebSocketError::Protocol(
                            "Received SnapshotResponse before Hello - protocol violation"
                                .to_string(),
                        ));
                    }
                    // Count ignored messages to prevent infinite loops
                    ignored_message_count += 1;
                    GPS_DEBUG!(
                        "Ignoring non-Hello message: {} ({}/{})",
                        typed_msg.msg_type,
                        ignored_message_count,
                        MAX_IGNORED_MESSAGES
                    );
                    if ignored_message_count > MAX_IGNORED_MESSAGES {
                        close_websocket(&mut websocket);
                        return Err(WebSocketError::Protocol(
                            "Too many non-Hello messages received".to_string(),
                        ));
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                GPS_DEBUG!("Received Ping, sending Pong");
                if let Err(e) = websocket.send(Message::Pong(data)) {
                    GPS_DEBUG!("Failed to send Pong: {}", e);
                }
                continue;
            }
            Ok(Message::Close(frame)) => {
                GPS_DEBUG!("Received Close frame: {:?}", frame);
                close_websocket(&mut websocket);
                return Err(WebSocketError::Connection(
                    "Peer closed connection".to_string(),
                ));
            }
            Ok(_) => continue,
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // Timeout, check cancellation and retry
                continue;
            }
            Err(e) => {
                close_websocket(&mut websocket);
                return Err(WebSocketError::Protocol(format!(
                    "Error reading message: {}",
                    e
                )));
            }
        }
    }

    // Send Snapshot request (server mode - no id needed)
    let snapshot_req = SnapshotRequest {
        id: None,
        msg_type: "Snapshot".to_string(),
    };
    let req_json = serde_json::to_string(&snapshot_req)
        .map_err(|e| WebSocketError::Protocol(format!("Failed to serialize request: {}", e)))?;
    GPS_INFO!("Sending: {}", req_json);

    websocket
        .send(Message::Text(req_json))
        .map_err(|e| WebSocketError::Protocol(format!("Failed to send Snapshot request: {}", e)))?;

    // Wait for SnapshotResponse
    loop {
        if handle.is_cancelled() {
            close_websocket(&mut websocket);
            return Err(WebSocketError::Cancelled);
        }
        if start_time.elapsed() > OPERATION_TIMEOUT {
            close_websocket(&mut websocket);
            return Err(WebSocketError::Connection(
                "Operation timed out".to_string(),
            ));
        }

        match websocket.read() {
            Ok(Message::Text(text)) => {
                // Check message size to prevent DoS
                if text.len() > MAX_MESSAGE_SIZE {
                    close_websocket(&mut websocket);
                    return Err(WebSocketError::Protocol(format!(
                        "Message too large: {} bytes (max: {} bytes)",
                        text.len(),
                        MAX_MESSAGE_SIZE
                    )));
                }

                GPS_DEBUG!("Received: {}", truncate_for_log(&text, 200));

                if let Ok(response) = serde_json::from_str::<SnapshotResponse>(&text) {
                    if response.msg_type == "SnapshotResponse" {
                        // Warn if multiple pipelines present (only first is used)
                        if response.pipelines.len() > 1 {
                            GPS_WARN!(
                                "Received {} pipelines, only loading first",
                                response.pipelines.len()
                            );
                        }
                        if let Some(pipeline) = response.pipelines.into_iter().next() {
                            if let Some(dot_content) = pipeline.dot {
                                GPS_INFO!("Got DOT content, loading pipeline");
                                close_websocket(&mut websocket);
                                return Ok(dot_content);
                            } else {
                                close_websocket(&mut websocket);
                                return Err(WebSocketError::Protocol(
                                    "Pipeline has no DOT content".to_string(),
                                ));
                            }
                        } else {
                            close_websocket(&mut websocket);
                            return Err(WebSocketError::Protocol(
                                "No pipelines in response".to_string(),
                            ));
                        }
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                GPS_DEBUG!("Received Ping, sending Pong");
                if let Err(e) = websocket.send(Message::Pong(data)) {
                    GPS_DEBUG!("Failed to send Pong: {}", e);
                }
                continue;
            }
            Ok(Message::Close(frame)) => {
                GPS_DEBUG!("Received Close frame: {:?}", frame);
                close_websocket(&mut websocket);
                return Err(WebSocketError::Connection(
                    "Peer closed connection".to_string(),
                ));
            }
            Ok(_) => continue,
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => {
                close_websocket(&mut websocket);
                return Err(WebSocketError::Protocol(format!(
                    "Error reading message: {}",
                    e
                )));
            }
        }
    }
}

/// Truncate a string for logging (to avoid flooding logs with large DOT content).
/// Uses character-based truncation to avoid panicking on UTF-8 boundaries.
fn truncate_for_log(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    if truncated.len() == s.len() {
        truncated
    } else {
        format!("{}... ({} bytes total)", truncated, s.len())
    }
}
