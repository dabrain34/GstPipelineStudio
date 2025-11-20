// elements.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

//! GStreamer element and node management operations.
//!
//! Provides functionality for creating and managing GStreamer elements as visual nodes,
//! including port (pad) creation, property updates, and link management. Handles special
//! cases for URI source/sink elements with file chooser dialogs.

use gtk::glib;
use std::collections::HashMap;

use crate::gps as GPS;
use crate::graphmanager as GM;
use crate::graphmanager::PropertyExt;
use crate::logger;
use crate::ui as GPSUI;
use crate::GPS_DEBUG;

use super::super::GPSApp;
use super::graphbook;

impl GPSApp {
    pub fn add_new_element(&self, element_name: &str) {
        let (inputs, outputs) = GPS::PadInfo::pads(element_name, false);
        let node = graphbook::current_graphtab(self)
            .graphview()
            .create_node(element_name, GPS::ElementInfo::element_type(element_name));
        let node_id = node.id();
        if let Some((prop_name, file_chooser)) =
            GPS::ElementInfo::element_is_uri_src_handler(element_name)
        {
            if file_chooser {
                GPSUI::dialog::get_file(
                    self,
                    GPSUI::dialog::FileDialogType::OpenAll,
                    move |app, filename| {
                        GPS_DEBUG!("Open file {}", filename);
                        let mut properties: HashMap<String, String> = HashMap::new();
                        properties.insert(prop_name.clone(), filename);
                        if let Some(node) =
                            graphbook::current_graphtab(&app).graphview().node(node_id)
                        {
                            node.update_properties(&properties);
                        }
                    },
                );
            } else {
                GPSUI::dialog::get_input(self, "Enter uri", "uri", "", move |app, uri| {
                    GPS_DEBUG!("Open uri {}", uri);
                    let mut properties: HashMap<String, String> = HashMap::new();
                    properties.insert(String::from("uri"), uri);
                    if let Some(node) = graphbook::current_graphtab(&app).graphview().node(node_id)
                    {
                        node.update_properties(&properties);
                    }
                });
            }
        } else if let Some((prop_name, file_chooser)) =
            GPS::ElementInfo::element_is_uri_sink_handler(element_name)
        {
            if file_chooser {
                GPSUI::dialog::get_file(
                    self,
                    GPSUI::dialog::FileDialogType::SaveAll,
                    move |app, filename| {
                        GPS_DEBUG!("Save file {}", filename);
                        let mut properties: HashMap<String, String> = HashMap::new();
                        properties.insert(prop_name.clone(), filename);
                        if let Some(node) =
                            graphbook::current_graphtab(&app).graphview().node(node_id)
                        {
                            node.update_properties(&properties);
                        }
                    },
                );
            } else {
                GPSUI::dialog::get_input(self, "Enter uri", "uri", "", move |app, uri| {
                    GPS_DEBUG!("Save uri {}", uri);
                    let mut properties: HashMap<String, String> = HashMap::new();
                    properties.insert(String::from("uri"), uri);
                    if let Some(node) = graphbook::current_graphtab(&app).graphview().node(node_id)
                    {
                        node.update_properties(&properties);
                    }
                });
            }
        }
        graphbook::current_graphtab(self).graphview().add_node(node);
        for input in inputs {
            self.create_port_with_caps(
                node_id,
                GM::PortDirection::Input,
                GM::PortPresence::Always,
                input.caps().unwrap_or("ANY").to_string(),
            );
        }
        for output in outputs {
            self.create_port_with_caps(
                node_id,
                GM::PortDirection::Output,
                GM::PortPresence::Always,
                output.caps().unwrap_or("ANY").to_string(),
            );
        }
    }

    pub fn node(&self, node_id: u32) -> GM::Node {
        let node = graphbook::current_graphtab(self)
            .graphview()
            .node(node_id)
            .unwrap_or_else(|| panic!("Unable to retrieve node with id {}", node_id));
        node
    }

    pub fn port(&self, node_id: u32, port_id: u32) -> GM::Port {
        let node = self.node(node_id);
        node.port(port_id)
            .unwrap_or_else(|| panic!("Unable to retrieve port with id {}", port_id))
    }

    pub fn update_element_properties(&self, node_id: u32, properties: &HashMap<String, String>) {
        let node = self.node(node_id);
        node.update_properties(properties);

        // Trigger graph update to save to cache file
        graphbook::current_graphtab(self)
            .graphview()
            .graph_updated();
    }

    pub fn update_pad_properties(
        &self,
        node_id: u32,
        port_id: u32,
        properties: &HashMap<String, String>,
    ) {
        let port = self.port(node_id, port_id);
        port.update_properties(properties);

        // Trigger graph update to save to cache file
        graphbook::current_graphtab(self)
            .graphview()
            .graph_updated();
    }

    pub fn element_property(&self, node_id: u32, property_name: &str) -> Option<String> {
        let node = self.node(node_id);
        PropertyExt::property(&node, property_name)
    }

    pub fn pad_properties(&self, node_id: u32, port_id: u32) -> HashMap<String, String> {
        let port = self.port(node_id, port_id);
        let mut properties: HashMap<String, String> = HashMap::new();
        for (name, value) in port.properties().iter() {
            if !port.hidden_property(name) {
                properties.insert(name.to_string(), value.to_string());
            }
        }
        properties
    }

    pub fn create_port_with_caps(
        &self,
        node_id: u32,
        direction: GM::PortDirection,
        presence: GM::PortPresence,
        caps: String,
    ) -> u32 {
        let node = self.node(node_id);
        let ports = node.all_ports(direction);
        let port_name = match direction {
            GM::PortDirection::Input => String::from("sink_"),
            GM::PortDirection::Output => String::from("src_"),
            _ => String::from("?"),
        };
        let port_name = format!("{}{}", port_name, ports.len());
        let port = graphbook::current_graphtab(self)
            .graphview()
            .create_port(&port_name, direction, presence);
        let id = port.id();
        let properties: HashMap<String, String> = HashMap::from([("_caps".to_string(), caps)]);
        port.update_properties(&properties);
        if let Some(mut node) = graphbook::current_graphtab(self).graphview().node(node_id) {
            graphbook::current_graphtab(self)
                .graphview()
                .add_port_to_node(&mut node, port);
        }
        id
    }

    pub fn create_link(
        &self,
        node_from_id: u32,
        node_to_id: u32,
        port_from_id: u32,
        port_to_id: u32,
    ) {
        let graphtab = graphbook::current_graphtab(self);
        let link =
            graphtab
                .graphview()
                .create_link(node_from_id, node_to_id, port_from_id, port_to_id);
        graphtab.graphview().add_link(link);
    }
}
