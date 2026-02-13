// mod.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use glib::SignalHandlerId;
use gtk::glib;
use gtk::prelude::*;
use gtk::{ApplicationWindow, Builder};
use log::error;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops;
use std::rc::{Rc, Weak};

use crate::logger;
use crate::ui as GPSUI;
use crate::GPS_DEBUG;
use crate::GPS_ERROR;
use crate::GPS_INFO;
use std::fmt;

// Submodules
pub mod core;
pub mod settings;

// Re-export commonly used types
pub use settings::Settings;

// Re-export commonly used constants from core panels module
pub use core::panels::{
    DEFAULT_PANED_POSITION, PANED_ELEMENTS_PREVIEW, PANED_ELEMENTS_PROPERTIES,
    PANED_GRAPH_DASHBOARD, PANED_GRAPH_LOGS,
};

const MAXIMIZE_TIMEOUT_MS: u64 = 500;

/// Minimum time the splash screen is displayed, even if GStreamer initializes faster.
/// This ensures users see the splash branding and don't experience a jarring flash.
pub const SPLASH_MIN_DISPLAY_MS: u64 = 1500;

#[derive(Debug)]
pub struct GPSAppInner {
    pub window: gtk::ApplicationWindow,
    pub current_graphtab: Cell<u32>,
    pub graphbook: RefCell<HashMap<u32, core::graphbook::GraphTab>>,
    pub builder: Builder,
    pub signal_handlers: RefCell<HashMap<String, SignalHandlerId>>,
}

#[derive(Debug, PartialEq)]
pub enum AppState {
    Ready,
    Playing,
    Paused,
    Stopped,
    Error(Option<String>),
}

impl fmt::Display for AppState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppState::Error(_) => write!(f, "Error"),
            _ => write!(f, "{self:?}"),
        }
    }
}

// This represents our main application window.
#[derive(Debug, Clone)]
pub struct GPSApp(Rc<GPSAppInner>);

// Deref into the contained struct to make usage a bit more ergonomic
impl ops::Deref for GPSApp {
    type Target = GPSAppInner;

    fn deref(&self) -> &GPSAppInner {
        &self.0
    }
}

// Weak reference to our application struct
//
// Weak references are important to prevent reference cycles. Reference cycles are cases where
// struct A references directly or indirectly struct B, and struct B references struct A again
// while both are using reference counting.
pub struct GPSAppWeak(Weak<GPSAppInner>);
impl GPSAppWeak {
    // Upgrade to a strong reference if it still exists
    pub fn upgrade(&self) -> Option<GPSApp> {
        self.0.upgrade().map(GPSApp)
    }
}

impl GPSApp {
    fn new(application: &gtk::Application) -> anyhow::Result<GPSApp> {
        let glade_src = include_str!("../ui/gps.ui");
        let builder = Builder::from_string(glade_src);
        let window: ApplicationWindow = builder
            .object("mainwindow")
            .expect("Couldn't get the main window");
        window.set_application(Some(application));
        window.set_title(Some("GStreamer Pipeline Studio"));

        let app = GPSApp(Rc::new(GPSAppInner {
            window,
            current_graphtab: Cell::new(0),
            graphbook: RefCell::new(HashMap::new()),
            builder,
            signal_handlers: RefCell::new(HashMap::new()),
        }));
        let settings = Settings::load_settings();

        app.window
            .set_default_size(settings.app_width, settings.app_height);

        if settings.app_maximized {
            app.window.maximize();
        }

        app.set_paned_position(&settings, PANED_GRAPH_DASHBOARD, DEFAULT_PANED_POSITION);
        app.set_paned_position(&settings, PANED_GRAPH_LOGS, DEFAULT_PANED_POSITION);
        app.set_paned_position(&settings, PANED_ELEMENTS_PREVIEW, DEFAULT_PANED_POSITION);
        app.set_paned_position(&settings, PANED_ELEMENTS_PROPERTIES, DEFAULT_PANED_POSITION);

        Ok(app)
    }

    /// Creates and shows the main window with hidden content.
    /// This is phase 1 of startup - the window is presented but with opacity 0
    /// to avoid showing improperly positioned UI (paned widgets at default positions)
    /// during the stabilization delay.
    /// Call `reveal_window()` when ready to show content (just before splash).
    pub fn create_window(application: &gtk::Application) -> Option<GPSApp> {
        // Apply system-wide dark theme early so splash screen inherits it
        if Settings::dark_theme() {
            if let Some(gtk_settings) = gtk::Settings::default() {
                gtk_settings.set_gtk_application_prefer_dark_theme(true);
            }
        }

        match GPSApp::new(application) {
            Ok(app) => {
                // Present window but make it invisible via opacity.
                // This ensures the window is properly mapped (required for transient splash)
                // while hiding the improperly positioned UI during startup.
                app.window.set_opacity(0.0);
                app.window.present();
                Some(app)
            }
            Err(err) => {
                error!("Error creating application: {}", err);
                None
            }
        }
    }

    /// Reveals the main window content. Call this just before showing the splash.
    pub fn reveal_window(&self) {
        self.window.set_opacity(1.0);
    }

    /// Initializes the UI content and sets up signal handlers.
    /// This is phase 2 of startup - called after GStreamer has initialized.
    pub fn initialize_ui(self, application: &gtk::Application, pipeline_desc: &String) {
        self.build_ui(application, pipeline_desc);

        // Apply paned positions after UI is built and allocated
        let app_for_paned = self.clone();
        let is_maximized = self.window.is_maximized();
        glib::timeout_add_local_once(
            std::time::Duration::from_millis(MAXIMIZE_TIMEOUT_MS),
            move || {
                app_for_paned.apply_paned_positions(is_maximized);
            },
        );

        self.setup_signal_handlers(application);
    }

    fn setup_signal_handlers(self, application: &gtk::Application) {
        let app = self;

        // Setup dynamic paned positioning on maximize/unmaximize
        let app_clone_for_maximize = app.clone();
        let last_maximized_state = Rc::new(Cell::new(app.window.is_maximized()));

        app.window
            .connect_notify_local(Some("maximized"), move |window, _| {
                let is_maximized = window.is_maximized();

                // Only process if state actually changed
                if last_maximized_state.get() == is_maximized {
                    return;
                }
                last_maximized_state.set(is_maximized);

                let app = app_clone_for_maximize.clone();

                // Use timeout to ensure window is fully resized and allocated
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(MAXIMIZE_TIMEOUT_MS),
                    move || {
                        app.apply_paned_positions(is_maximized);
                    },
                );
            });

        // Setup dynamic paned positioning on window resize (for windowed mode)
        let app_clone_for_resize = app.clone();
        let resize_timeout_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

        app.window.connect_default_width_notify(glib::clone!(
            #[strong]
            resize_timeout_id,
            #[strong]
            app_clone_for_resize,
            move |window| {
                // Only apply resize in windowed mode (not when maximized)
                if window.is_maximized() {
                    return;
                }

                // Cancel any pending resize timeout
                if let Some(id) = resize_timeout_id.borrow_mut().take() {
                    id.remove();
                }

                let app = app_clone_for_resize.clone();
                let timeout_id_clone = resize_timeout_id.clone();

                // Use timeout to debounce resize events
                let new_id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(MAXIMIZE_TIMEOUT_MS),
                    move || {
                        app.apply_paned_positions(false);
                        timeout_id_clone.borrow_mut().take();
                    },
                );
                *resize_timeout_id.borrow_mut() = Some(new_id);
            }
        ));

        app.window.connect_default_height_notify(glib::clone!(
            #[strong]
            resize_timeout_id,
            #[strong]
            app_clone_for_resize,
            move |window| {
                // Only apply resize in windowed mode (not when maximized)
                if window.is_maximized() {
                    return;
                }

                // Cancel any pending resize timeout
                if let Some(id) = resize_timeout_id.borrow_mut().take() {
                    id.remove();
                }

                let app = app_clone_for_resize.clone();
                let timeout_id_clone = resize_timeout_id.clone();

                // Use timeout to debounce resize events
                let new_id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(MAXIMIZE_TIMEOUT_MS),
                    move || {
                        app.apply_paned_positions(false);
                        timeout_id_clone.borrow_mut().take();
                    },
                );
                *resize_timeout_id.borrow_mut() = Some(new_id);
            }
        ));

        let timeout_id = app.setup_position_slider();

        let timeout_id = RefCell::new(Some(timeout_id));
        let app_container = RefCell::new(Some(app));

        application.connect_shutdown(move |_| {
            let app = app_container
                .borrow_mut()
                .take()
                .expect("Shutdown called multiple times");
            let window: ApplicationWindow = app
                .builder
                .object("mainwindow")
                .expect("Couldn't get the main window");
            let mut settings = Settings::load_settings();
            settings.app_maximized = window.is_maximized();
            settings.app_width = window.default_width();
            settings.app_height = window.default_height();
            app.save_paned_position(&mut settings, PANED_GRAPH_DASHBOARD);
            app.save_paned_position(&mut settings, PANED_GRAPH_LOGS);
            app.save_paned_position(&mut settings, PANED_ELEMENTS_PREVIEW);
            app.save_paned_position(&mut settings, PANED_ELEMENTS_PROPERTIES);

            Settings::mark_clean_shutdown(&mut settings);
            Settings::save_settings(&settings);
            if let Some(timeout_id) = timeout_id.borrow_mut().take() {
                timeout_id.remove();
            }

            app.drop();
        });
    }

    // Downgrade to a weak reference
    pub fn downgrade(&self) -> GPSAppWeak {
        GPSAppWeak(Rc::downgrade(&self.0))
    }

    /// Start GPS as WebSocket server for pipeline-snapshot tracer.
    /// The tracer connects to GPS and GPS sends Snapshot request to get DOT.
    /// Use with: GST_TRACERS="pipeline-snapshot(dots-viewer-ws-url=ws://HOST:PORT)"
    pub fn start_websocket_server(&self, ws_addr: &str) {
        use crate::gps::websocket::WebSocketError;

        // Shared handle - set immediately when run_server returns (before thread spawns)
        let server_handle: Rc<RefCell<Option<crate::gps::websocket::ServerHandle>>> =
            Rc::new(RefCell::new(None));
        let server_handle_for_cancel = server_handle.clone();

        // Show waiting dialog with Cancel button
        let waiting_dialog = GPSUI::dialog::show_waiting(
            self,
            "Listen for Pipeline",
            &format!("Listening on {}...", ws_addr),
            move || {
                GPS_DEBUG!("Cancel callback triggered");
                // Cancel the server if handle exists
                if let Some(handle) = server_handle_for_cancel.borrow().as_ref() {
                    GPS_DEBUG!("Cancelling server...");
                    handle.cancel();
                } else {
                    GPS_DEBUG!("No server handle to cancel yet");
                }
            },
        );

        // Clone for the completion callback
        let dialog_for_completion = waiting_dialog.clone();

        // run_server now returns synchronously with the handle, fixing the race condition
        match crate::gps::websocket::run_server(ws_addr, self.downgrade(), move |result| {
            // Close dialog when server completes
            dialog_for_completion.close();

            match result {
                Ok(()) => {
                    GPS_INFO!("WebSocket server completed successfully");
                }
                Err(WebSocketError::Cancelled) => {
                    // User cancelled, no need to log as error
                    GPS_DEBUG!("WebSocket server cancelled by user");
                }
                Err(e) => {
                    GPS_ERROR!("WebSocket server error: {}", e);
                }
            }
        }) {
            Ok(handle) => {
                // Store handle immediately so cancel button can use it
                *server_handle.borrow_mut() = Some(handle);
            }
            Err(e) => {
                waiting_dialog.close();
                GPS_ERROR!("Failed to start WebSocket server: {}", e);
            }
        }
    }

    /// Load DOT content string into the current graph view
    pub fn load_dot_content(&self, dot_content: &str) {
        use crate::gps::GstDotLoader;

        let graphtab = core::graphbook::current_graphtab(self);
        let graphview = graphtab.graphview();

        // Clear the current graph before loading new content
        graphview.clear();

        // Load the DOT content using GstDotLoader
        if let Err(e) = graphview.load_from_dot(dot_content, &GstDotLoader) {
            GPS_ERROR!("Failed to load DOT content: {}", e);
            GPSUI::message::display_error_dialog(
                false,
                &format!("Failed to load pipeline graph: {}", e),
            );
        }
    }

    // Called when the application shuts down. We drop our app struct here
    fn drop(self) {}
}
