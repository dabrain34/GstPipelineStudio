// dot.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

//! GStreamer-specific DOT loader implementation.
//!
//! This module provides `GstDotLoader`, a `DotLoader` trait implementation
//! that enhances DOT file loading with GStreamer-specific knowledge:
//! - Element type detection (Source, Sink, Transform, etc.)
//! - Factory existence validation
//! - Static pad information from element factories
//! - GStreamer-specific DOT format parsing (label format, ID format)

use crate::graphmanager::dot_parser::DotLoader;
use crate::graphmanager::NodeType;
use crate::logger;
use gtk::glib;
use std::collections::HashMap;

use super::{ElementInfo, PadInfo};

/// GStreamer DOT ID format constants and parsing utilities.
///
/// These constants define the prefixes and separators used in GStreamer's DOT output
/// for identifying nodes and ports. See GStreamer source: `gst/gstdebugutils.c`
///
/// The parsing functions are public so they can be reused in tests without
/// requiring GStreamer initialization.
pub mod dot_parsing {
    use std::borrow::Cow;
    use std::collections::HashMap;

    /// Prefix for node/port IDs in GStreamer 1.26+ format
    /// Format: "node_{instance}_{addr}_node_{padname}_{addr}"
    pub const NODE_PREFIX: &str = "node_";
    /// Length of NODE_PREFIX (5 characters)
    pub const NODE_PREFIX_LEN: usize = 5;
    /// Separator between element and pad sections in new format
    /// Appears as "_node_" between element info and pad info
    pub const NODE_SEPARATOR: &str = "_node_";
    /// Length of NODE_SEPARATOR (6 characters)
    pub const NODE_SEPARATOR_LEN: usize = 6;
    /// Memory address prefix used in DOT IDs
    /// Example: "_0x5d75d6505510"
    pub const ADDR_PREFIX: &str = "_0x";

    /// GStreamer DOT cluster prefixes for element subgraphs.
    /// New format (1.26+): "cluster_node_{instance}_{address}"
    pub const CLUSTER_PREFIX_NEW: &str = "cluster_node_";
    /// Old format (1.24): "cluster_{instance}_{address}"
    pub const CLUSTER_PREFIX_OLD: &str = "cluster_";

    /// GStreamer DOT port subgraph suffixes.
    pub const PORT_SUFFIX_SINK: &str = "_sink";
    pub const PORT_SUFFIX_SRC: &str = "_src";

    /// Check if a DOT subgraph ID represents a GStreamer element node.
    ///
    /// GStreamer DOT format uses "cluster_" or "cluster_node_" prefixes for elements,
    /// but port subgraphs (ending with "_sink" or "_src") should not be treated as nodes.
    pub fn is_node_subgraph(id: &str) -> bool {
        (id.starts_with(CLUSTER_PREFIX_NEW) || id.starts_with(CLUSTER_PREFIX_OLD))
            && !id.ends_with(PORT_SUFFIX_SINK)
            && !id.ends_with(PORT_SUFFIX_SRC)
    }

    /// Check if a DOT subgraph ID represents a GStreamer port grouping.
    ///
    /// Port subgraphs group sink or source pads and end with "_sink" or "_src".
    pub fn is_port_subgraph(id: &str) -> bool {
        id.ends_with(PORT_SUFFIX_SINK) || id.ends_with(PORT_SUFFIX_SRC)
    }

    /// Convert GStreamer class name to factory type name.
    ///
    /// - Strips "Gst" prefix
    /// - Handles wrapper bin pattern (GstGLImageSinkBin -> glimagesink)
    pub fn class_to_type_name(class_name: &str) -> String {
        let name = class_name.strip_prefix("Gst").unwrap_or(class_name);
        let name = if name.ends_with("SinkBin") || name.ends_with("SrcBin") {
            name.strip_suffix("Bin").unwrap_or(name)
        } else {
            name
        };
        name.to_lowercase()
    }

    /// Parse a GStreamer DOT node label into metadata.
    ///
    /// Returns a HashMap containing "class_name", "instance_name", optionally "state",
    /// and any other properties found in the label.
    /// Use `validate_property` callback to filter/validate properties.
    pub fn parse_node_label<F>(label: &str, validate_property: F) -> HashMap<String, String>
    where
        F: Fn(&str, &str) -> bool,
    {
        let label = label.trim_matches('"');

        let normalized: Cow<str> = if label.contains("\\n") || label.contains("&#10;") {
            Cow::Owned(label.replace("\\n", "\n").replace("&#10;", "\n"))
        } else {
            Cow::Borrowed(label)
        };
        let lines: Vec<&str> = normalized.split('\n').collect();

        let first_line = lines.first().map(|l| l.trim()).unwrap_or("");
        let first_non_empty = lines.iter().find(|l| !l.trim().is_empty());
        let is_properties_only = (first_line.is_empty()
            && first_non_empty.map(|l| l.contains('=')).unwrap_or(false))
            || first_line.contains('=');

        let mut metadata = HashMap::new();

        let prop_start = if is_properties_only {
            0
        } else {
            let class_name = lines.first().unwrap_or(&"Unknown").to_string();
            let instance_name = lines.get(1).unwrap_or(&"unknown").to_string();
            metadata.insert("class_name".to_string(), class_name);
            metadata.insert("instance_name".to_string(), instance_name);

            // Check for state on line 3 (format: "[PLAYING]")
            if let Some(state) = lines.get(2).and_then(|s| {
                let s = s.trim();
                s.strip_prefix('[')
                    .and_then(|s| s.strip_suffix(']'))
                    .map(|s| s.to_string())
            }) {
                metadata.insert("state".to_string(), state);
                3
            } else {
                2
            }
        };

        for line in lines.iter().skip(prop_start) {
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                // Skip runtime values (memory pointers, contexts, samples)
                if value.contains("0x")
                    || value.starts_with("((")
                    || value.starts_with("(Gst")
                    || key == "context"
                    || key == "last-sample"
                {
                    continue;
                }

                let value = value
                    .trim_start_matches('\\')
                    .trim_end_matches('\\')
                    .trim_matches('"');

                if validate_property(key, value) {
                    metadata.insert(key.to_string(), value.to_string());
                }
            }
        }

        metadata
    }

    /// Extract port name from a GStreamer DOT port ID.
    ///
    /// Handles both GStreamer 1.26+ and 1.24 formats.
    pub fn extract_port_name_from_id(dot_id: &str) -> Option<String> {
        // Try new format first: find the last occurrence of "_node_"
        if let Some(last_node_idx) = dot_id.rfind(NODE_SEPARATOR) {
            let after_node = &dot_id[last_node_idx + NODE_SEPARATOR_LEN..];
            let end_idx = after_node.find(ADDR_PREFIX).unwrap_or(after_node.len());
            let port_name = &after_node[..end_idx];
            if !port_name.is_empty() {
                return Some(port_name.to_string());
            }
        }

        // Old format: ELEMENT_0x..._PADNAME_0x...
        let last_addr = dot_id.rfind(ADDR_PREFIX)?;
        let before_last_addr = &dot_id[..last_addr];

        let mut pos = before_last_addr.len();
        while pos > 0 {
            if let Some(underscore_pos) = before_last_addr[..pos].rfind('_') {
                let after_underscore = &before_last_addr[underscore_pos + 1..];
                if !after_underscore.starts_with("0x") {
                    let port_name = &before_last_addr[underscore_pos + 1..];
                    if !port_name.is_empty() {
                        return Some(port_name.to_string());
                    }
                }
                pos = underscore_pos;
            } else {
                break;
            }
        }

        None
    }

    /// Extract node instance name from a GStreamer DOT port ID.
    ///
    /// Handles both GStreamer 1.26+ and 1.24 formats.
    pub fn extract_node_instance_from_id(port_id: &str) -> Option<String> {
        // Try new format: "node_" followed by instance name
        if let Some(start) = port_id.find(NODE_PREFIX) {
            let after_node = &port_id[start + NODE_PREFIX_LEN..];
            let end = after_node
                .find(ADDR_PREFIX)
                .or_else(|| after_node.find(NODE_SEPARATOR))
                .unwrap_or(after_node.len());
            let instance = &after_node[..end];
            if !instance.is_empty() {
                return Some(instance.to_string());
            }
        }

        // Old format: instance name at start, ending at first "_0x"
        let end = port_id.find(ADDR_PREFIX)?;
        let instance = &port_id[..end];
        if instance.is_empty() {
            None
        } else {
            Some(instance.to_string())
        }
    }

    /// Extract gst_version from graph attributes.
    pub fn extract_graph_metadata(attributes: &[(String, String)]) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        for (name, value) in attributes {
            if name == "gst_version" {
                metadata.insert("gst_version".to_string(), value.clone());
            }
        }
        metadata
    }
}

/// GStreamer-specific DOT loader.
///
/// This loader provides GStreamer element factory information
/// for enhanced DOT file loading with proper element types and pad info.
pub struct GstDotLoader;

impl GstDotLoader {
    /// Validate property value for security and correctness.
    ///
    /// This prevents malicious DOT files from setting dangerous property values
    /// that could lead to security vulnerabilities when applied to GStreamer elements.
    ///
    /// Note: We allow absolute paths since GStreamer DOT exports contain legitimate
    /// file paths. This is acceptable because:
    /// 1. DOT files are typically generated by GStreamer itself, not user-provided
    /// 2. Properties are not directly executed, only passed to GStreamer elements
    /// 3. The main protection is against malformed data, not path restrictions
    fn is_valid_property_value(key: &str, value: &str) -> bool {
        // Maximum allowed property value length (prevents DoS via huge strings)
        // 16KB allows long URLs with query parameters while still protecting against abuse
        const MAX_PROPERTY_LENGTH: usize = 16 * 1024;

        // Check value length
        if value.len() > MAX_PROPERTY_LENGTH {
            return false;
        }

        // Check for null bytes (security: could be used to bypass checks)
        if value.contains('\0') {
            return false;
        }

        // Reject properties that could execute commands
        // Note: Standard GStreamer elements don't have these properties,
        // but custom elements might, so we block them for safety
        const FORBIDDEN_PROPERTIES: &[&str] = &["exec", "command", "script"];
        if FORBIDDEN_PROPERTIES.contains(&key) {
            return false;
        }

        // Warn if file path properties point to non-existing paths
        // Note: We don't reject these because:
        // 1. The file might be created later
        // 2. The DOT file might be from a different system
        // 3. We want to preserve the property value for the user to fix
        const FILE_PATH_PROPERTIES: &[&str] = &["location", "uri", "device"];
        if FILE_PATH_PROPERTIES.contains(&key) {
            // Check if it looks like a local file path (not a URL)
            if !value.starts_with("http://") && !value.starts_with("https://") {
                let path = std::path::Path::new(value);
                if !path.exists() {
                    GPS_WARN!(
                        "Property '{}' references non-existing path: '{}' (DOT file may be from different system)",
                        key,
                        value
                    );
                }
            }
        }

        // All checks passed
        true
    }
}

impl DotLoader for GstDotLoader {
    fn node_type(&self, type_name: &str) -> NodeType {
        ElementInfo::element_type(type_name)
    }

    fn node_exists(&self, type_name: &str) -> bool {
        ElementInfo::element_factory_exists(type_name)
    }

    fn get_static_ports(&self, type_name: &str) -> (Vec<String>, Vec<String>) {
        let (inputs, outputs) = PadInfo::pads(type_name, false);

        let input_names: Vec<String> = inputs
            .iter()
            .filter_map(|pad| {
                pad.name().map(|s| s.to_string()).or_else(|| {
                    GPS_TRACE!("Skipping pad without name in element: {}", type_name);
                    None
                })
            })
            .collect();

        let output_names: Vec<String> = outputs
            .iter()
            .filter_map(|pad| {
                pad.name().map(|s| s.to_string()).or_else(|| {
                    GPS_TRACE!("Skipping pad without name in element: {}", type_name);
                    None
                })
            })
            .collect();

        (input_names, output_names)
    }

    fn class_to_type_name(&self, class_name: &str) -> String {
        dot_parsing::class_to_type_name(class_name)
    }

    fn parse_node_label(&self, label: &str) -> HashMap<String, String> {
        dot_parsing::parse_node_label(label, Self::is_valid_property_value)
    }

    fn extract_port_name_from_id(&self, dot_id: &str) -> Option<String> {
        dot_parsing::extract_port_name_from_id(dot_id)
    }

    fn extract_node_instance_from_id(&self, port_id: &str) -> Option<String> {
        dot_parsing::extract_node_instance_from_id(port_id)
    }

    fn extract_graph_metadata(&self, attributes: &[(String, String)]) -> HashMap<String, String> {
        dot_parsing::extract_graph_metadata(attributes)
    }

    fn is_node_subgraph(&self, id: &str) -> bool {
        dot_parsing::is_node_subgraph(id)
    }

    fn is_port_subgraph(&self, id: &str) -> bool {
        dot_parsing::is_port_subgraph(id)
    }
}

// Note: Tests for GstDotLoader are in gps/test.rs
// They require GStreamer initialization via test_synced()
