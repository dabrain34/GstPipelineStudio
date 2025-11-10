// logger.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::GPSApp;
use crate::logger;
use crate::ui::models::LogEntry;
use gtk::prelude::*;
use gtk::{gio, glib};

use gtk::{ColumnView, ColumnViewColumn, SignalListItemFactory, SingleSelection};

// Helper function to create a column for ColumnView
fn create_column_view_column(title: &str, property: &str) -> ColumnViewColumn {
    let factory = SignalListItemFactory::new();
    let property_name = property.to_string();
    let property_name_clone = property_name.clone();

    factory.connect_setup(move |_, list_item| {
        let label = gtk::Label::new(None);
        label.set_halign(gtk::Align::Start);
        label.set_margin_start(4);
        label.set_margin_end(4);
        list_item
            .downcast_ref::<gtk::ListItem>()
            .expect("Needs to be ListItem")
            .set_child(Some(&label));
    });

    factory.connect_bind(move |_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk::ListItem>()
            .expect("Needs to be ListItem");
        let log_entry = list_item
            .item()
            .and_downcast::<LogEntry>()
            .expect("The item has to be a LogEntry");
        let label = list_item
            .child()
            .and_downcast::<gtk::Label>()
            .expect("The child has to be a Label");

        let text = log_entry.property::<String>(&property_name_clone);
        label.set_text(&text);
    });

    let column = ColumnViewColumn::new(Some(title), Some(factory));
    column.set_expand(&property_name == "log"); // Expand the log column
    column
}

// Reset loggers (ColumnView)
fn reset_logger_column_view(column_view: &ColumnView) {
    let model = gio::ListStore::new::<LogEntry>();
    let selection_model = SingleSelection::new(Some(model));
    column_view.set_model(Some(&selection_model));
}

pub fn setup_logger_list(app: &GPSApp, logger_name: &str, log_type: logger::LogType) {
    // All loggers now use ColumnView
    let column_view: ColumnView = app
        .builder
        .object(logger_name)
        .expect("Couldn't get columnview");

    // Add columns based on logger type
    match log_type {
        logger::LogType::App | logger::LogType::Message => {
            column_view.append_column(&create_column_view_column("TIME", "time"));
            column_view.append_column(&create_column_view_column("LEVEL", "level"));
            column_view.append_column(&create_column_view_column("LOG", "log"));
        }
        logger::LogType::Gst => {
            column_view.append_column(&create_column_view_column("TIME", "time"));
            column_view.append_column(&create_column_view_column("LEVEL", "level"));
            column_view.append_column(&create_column_view_column("CATEGORY", "category"));
            column_view.append_column(&create_column_view_column("FILE", "file"));
            column_view.append_column(&create_column_view_column("LOG", "log"));
        }
    }

    reset_logger_column_view(&column_view);

    // Add context menu gesture
    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);
    let app_weak = app.downgrade();
    gesture.connect_pressed(glib::clone!(
        #[weak]
        column_view,
        move |gesture, _n_press, x, y| {
            let app = upgrade_weak!(app_weak);
            if gesture.current_button() == gtk::gdk::BUTTON_SECONDARY {
                let menu: gio::MenuModel = app
                    .builder
                    .object("logger_menu")
                    .expect("Couldn't get logger_menu model");

                let column_view_clone = column_view.clone();
                app.connect_app_menu_action("logger.clear", move |_, _| {
                    reset_logger_column_view(&column_view_clone);
                });

                app.show_context_menu_at_position(&column_view, x, y, &menu);
            }
        }
    ));
    column_view.add_controller(gesture);
}

fn log_tree_id_from_log_type(log_type: logger::LogType) -> String {
    match log_type {
        logger::LogType::App => String::from("treeview-app-logger"),
        logger::LogType::Gst => String::from("treeview-gst-logger"),
        logger::LogType::Message => String::from("treeview-msg-logger"),
    }
}

pub fn add_to_logger_list(app: &GPSApp, log_type: logger::LogType, log_entry: &str) {
    let log_tree_name = log_tree_id_from_log_type(log_type.clone());

    // All loggers now use ColumnView
    let column_view: ColumnView = app
        .builder
        .object(log_tree_name.as_str())
        .expect("Couldn't get columnview");

    if let Some(model) = column_view.model() {
        let selection_model = model
            .downcast_ref::<SingleSelection>()
            .expect("Could not cast to SingleSelection");
        if let Some(list_model) = selection_model.model() {
            let list_store = list_model
                .downcast_ref::<gio::ListStore>()
                .expect("Could not cast to gio::ListStore");

            // Parse log entry based on type
            let entry = match log_type {
                logger::LogType::Gst => {
                    let log: Vec<&str> = log_entry.splitn(5, '\t').collect();
                    LogEntry::new(
                        log.first().unwrap_or(&""),
                        log.get(1).unwrap_or(&""),
                        log.get(2).unwrap_or(&""),
                        log.get(3).unwrap_or(&""),
                        log.get(4).unwrap_or(&""),
                    )
                }
                logger::LogType::App | logger::LogType::Message => {
                    let log: Vec<&str> = log_entry.splitn(3, ' ').collect();
                    LogEntry::new_simple(
                        log.first().unwrap_or(&""),
                        log.get(1).unwrap_or(&""),
                        log.get(2).unwrap_or(&""),
                    )
                }
            };

            // Append to end (newest at bottom)
            list_store.append(&entry);

            // Auto-scroll to the last item
            let n_items = list_store.n_items();
            if n_items > 0 {
                selection_model.set_selected(n_items - 1);
                // Use idle_add to ensure scroll happens after render
                let column_view_clone = column_view.clone();
                let selection_clone = selection_model.clone();
                glib::idle_add_local_once(move || {
                    // Scroll to the selected item (last item)
                    if let Some(widget) = column_view_clone.first_child() {
                        if let Some(scrolled) = widget.ancestor(gtk::ScrolledWindow::static_type())
                        {
                            if let Some(scrolled) = scrolled.downcast_ref::<gtk::ScrolledWindow>() {
                                let vadj = scrolled.vadjustment();
                                vadj.set_value(vadj.upper() - vadj.page_size());
                            }
                        }
                    }
                    // Deselect to avoid highlighting
                    selection_clone.set_selected(gtk::INVALID_LIST_POSITION);
                });
            }
        }
    }
}
