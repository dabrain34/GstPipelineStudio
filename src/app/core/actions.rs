// actions.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

//! Application actions and keyboard shortcuts.
//!
//! This module manages all GTK application-level actions and their associated
//! keyboard shortcuts (accelerators) for GStreamer Pipeline Studio. It provides
//! functionality for:
//!
//! - Setting up application menu actions (file operations, graph operations, etc.)
//! - Managing recent files menu with automatic validation
//! - Connecting and disconnecting action handlers dynamically
//! - Binding keyboard shortcuts to actions
//!
//! # Actions Overview
//!
//! The module defines actions in several categories:
//!
//! ## File Operations
//! - `new-window` - Create new graph tab (<Ctrl+N>)
//! - `open` - Open graph file (<Ctrl+O>)
//! - `open_pipeline` - Open pipeline description (<Ctrl+P>)
//! - `save` - Save current graph (<Ctrl+S>)
//! - `save_as` - Save graph with new filename
//!
//! ## Graph Operations
//! - `graph.check` - Validate pipeline
//! - `graph.clear` - Clear current graph
//! - `graph.pipeline_details` - Show pipeline details (enabled only when playing)
//! - `delete` - Delete selected elements (<Ctrl+D> or Delete)
//!
//! ## Element Operations
//! - `node.add-to-favorite` - Add element to favorites
//! - `node.delete` - Delete node
//! - `node.request-pad-input` - Request input pad
//! - `node.request-pad-output` - Request output pad
//! - `node.properties` - Show element properties
//! - `node.duplicate` - Duplicate element
//! - `port.delete` - Delete port
//! - `port.properties` - Show port properties
//! - `link.delete` - Delete link
//!
//! ## Other
//! - `preferences` - Show preferences dialog (<Ctrl+P>)
//! - `about` - Show about dialog (<Ctrl+A>)
//! - `logger.clear` - Clear log messages
//!
//! # Recent Files
//!
//! The recent files menu dynamically updates to show the 4 most recently opened
//! files, automatically removing any that no longer exist on disk.

use gtk::glib;
use gtk::prelude::*;
use gtk::{gio, gio::SimpleAction, Button};
use std::path::Path;

use crate::logger;
use crate::GPS_ERROR;

use super::super::settings::Settings;
use super::super::GPSApp;

impl GPSApp {
    pub fn setup_app_actions(&self, application: &gtk::Application) {
        application.add_action(&gio::SimpleAction::new("new-window", None));
        application.set_accels_for_action("app.new-window", &["<primary>n"]);

        application.add_action(&gio::SimpleAction::new("open", None));
        application.set_accels_for_action("app.open", &["<primary>o"]);

        application.add_action(&gio::SimpleAction::new("open_pipeline", None));
        application.set_accels_for_action("app.open_pipeline", &["<primary>p"]);

        application.add_action(&gio::SimpleAction::new("save", None));
        application.set_accels_for_action("app.save", &["<primary>s"]);
        application.add_action(&gio::SimpleAction::new("save_as", None));

        application.add_action(&gio::SimpleAction::new("delete", None));
        application.set_accels_for_action("app.delete", &["<primary>d", "Delete", "BackSpace"]);

        application.add_action(&gio::SimpleAction::new("preferences", None));
        application.set_accels_for_action("app.preferences", &["<primary>p"]);

        application.add_action(&gio::SimpleAction::new("about", None));
        application.set_accels_for_action("app.about", &["<primary>a"]);

        application.add_action(&gio::SimpleAction::new("favorite.remove", None));
        application.add_action(&gio::SimpleAction::new("element.add-to-favorite", None));

        application.add_action(&gio::SimpleAction::new("logger.clear", None));

        application.add_action(&gio::SimpleAction::new("graph.check", None));
        application.add_action(&gio::SimpleAction::new("graph.clear", None));

        let pipeline_details_action = gio::SimpleAction::new("graph.pipeline_details", None);
        pipeline_details_action.set_enabled(false); // Initially disabled
        application.add_action(&pipeline_details_action);

        application.add_action(&gio::SimpleAction::new("port.delete", None));
        application.add_action(&gio::SimpleAction::new("port.properties", None));

        application.add_action(&gio::SimpleAction::new("link.delete", None));

        application.add_action(&gio::SimpleAction::new("node.add-to-favorite", None));
        application.add_action(&gio::SimpleAction::new("node.delete", None));
        application.add_action(&gio::SimpleAction::new("node.request-pad-input", None));
        application.add_action(&gio::SimpleAction::new("node.request-pad-output", None));
        application.add_action(&gio::SimpleAction::new("node.properties", None));
        application.add_action(&gio::SimpleAction::new("node.duplicate", None));
    }

    pub fn update_recent_files_menu(&self) {
        let application = gio::Application::default()
            .expect("No default application")
            .downcast::<gtk::Application>()
            .expect("Unable to downcast default application");

        // Get the recent files menu from the builder
        let recent_menu: gio::Menu = self
            .builder
            .object("recent_files_menu")
            .expect("Couldn't get recent_files_menu");

        // Clear existing menu items
        recent_menu.remove_all();

        // Clean up old recent file actions
        for i in 0..4 {
            let action_name = format!("recent_file_{}", i);
            if application.lookup_action(&action_name).is_some() {
                application.remove_action(&action_name);
            }
        }

        // Get recent files and filter out non-existent ones
        let recent_files = Settings::get_recent_open_files();
        let mut valid_files = Vec::new();

        for file in recent_files {
            if Path::new(&file).exists() {
                valid_files.push(file);
            }
        }

        // Update settings if we removed any invalid files
        if valid_files.len() < Settings::get_recent_open_files().len() {
            let mut settings = Settings::load_settings();
            settings.recent_open_files = valid_files.clone();
            Settings::save_settings(&settings);
        }

        // Populate the menu
        if valid_files.is_empty() {
            let item = gio::MenuItem::new(Some("(No recent files)"), None);
            recent_menu.append_item(&item);
        } else {
            for (i, filename) in valid_files.iter().enumerate().take(4) {
                let action_name = format!("recent_file_{}", i);
                let full_action_name = format!("app.{}", action_name);

                // Extract just the filename for display
                let display_name = Path::new(filename)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(filename);

                let item = gio::MenuItem::new(Some(display_name), Some(&full_action_name));
                recent_menu.append_item(&item);

                // Create action for this recent file
                let action = gio::SimpleAction::new(&action_name, None);
                let app_weak = self.downgrade();
                let filename_clone = filename.clone();

                action.connect_activate(move |_, _| {
                    let app = upgrade_weak!(app_weak);
                    app.load_graph(&filename_clone, false)
                        .unwrap_or_else(|_| GPS_ERROR!("Unable to open file {}", filename_clone));
                    app.update_recent_files_menu();
                });

                application.add_action(&action);
            }
        }
    }

    pub fn app_menu_action(&self, action_name: &str) -> SimpleAction {
        let application = gio::Application::default()
            .expect("No default application")
            .downcast::<gtk::Application>()
            .expect("Unable to downcast default application");

        application
            .lookup_action(action_name)
            .unwrap_or_else(|| panic!("Unable to find action {}", action_name))
            .dynamic_cast::<SimpleAction>()
            .expect("Unable to dynamic cast to SimpleAction")
    }

    pub fn disconnect_app_menu_action(&self, action_name: &str) {
        let action = self.app_menu_action(action_name);

        if let Some(signal_handler_id) = self.signal_handlers.borrow_mut().remove(action_name) {
            action.disconnect(signal_handler_id);
        }
    }

    pub fn connect_app_menu_action<
        F: Fn(&SimpleAction, std::option::Option<&glib::Variant>) + 'static,
    >(
        &self,
        action_name: &str,
        f: F,
    ) {
        let action = self.app_menu_action(action_name);
        self.disconnect_app_menu_action(action_name);
        let signal_handler_id = action.connect_activate(f);
        self.signal_handlers
            .borrow_mut()
            .insert(String::from(action_name), signal_handler_id);
    }

    pub fn connect_button_action<F: Fn(&Button) + 'static>(&self, button_name: &str, f: F) {
        let button: Button = self
            .builder
            .object(button_name)
            .unwrap_or_else(|| panic!("Couldn't get app_button {}", button_name));
        button.connect_clicked(f);
    }
}
