// menu.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

//! Context menu operations and popover management.
//!
//! Provides functionality for displaying context menus at specific coordinates
//! using GTK4's PopoverMenu. Used for right-click menus on graph, nodes, and ports.

use gtk::gdk;
use gtk::prelude::*;
use gtk::{gio, PopoverMenu, Widget};

use super::super::GPSApp;

impl GPSApp {
    pub fn app_pop_menu_at_position(
        &self,
        widget: &impl IsA<Widget>,
        x: f64,
        y: f64,
        menu_model: Option<&gio::MenuModel>,
    ) -> PopoverMenu {
        // Create a new PopoverMenu dynamically for GTK4
        let popover = PopoverMenu::builder().has_arrow(false).build();

        // Set the menu model if provided
        if let Some(model) = menu_model {
            popover.set_menu_model(Some(model));
        }

        // Set parent widget
        popover.set_parent(widget);

        // Set positioning
        let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
        popover.set_pointing_to(Some(&rect));

        // Use popup() which is the correct GTK4 method for context menus
        popover.popup();

        popover
    }

    pub fn show_context_menu_at_position(
        &self,
        widget: &impl IsA<Widget>,
        x: f64,
        y: f64,
        menu_model: &gio::MenuModel,
    ) {
        let popover = self.app_pop_menu_at_position(widget, x, y, Some(menu_model));

        // Set up auto-hide when menu item is activated
        popover.connect_closed(move |_| {
            // Context menu closed
        });
    }
}
