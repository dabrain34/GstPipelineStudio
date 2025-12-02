// logger.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::GPSApp;
use crate::logger;
use crate::ui::common::{create_column_view_column, create_column_view_column_with_width};
use crate::ui::models::LogEntry;
use gtk::prelude::*;
use gtk::{gio, glib};

use gtk::{ColumnView, SingleSelection};

// Column width constants
const COL_WIDTH_TIME: i32 = 80;
const COL_WIDTH_LEVEL: i32 = 80;
const COL_WIDTH_SRC: i32 = 150;
const COL_WIDTH_TYPE: i32 = 150;
const COL_WIDTH_FUNCTION: i32 = 300;
const COL_WIDTH_CATEGORY: i32 = 150;
const COL_WIDTH_FILE: i32 = 200;

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

    // Add columns based on logger type with appropriate widths
    match log_type {
        logger::LogType::App => {
            column_view.append_column(&create_column_view_column_with_width(
                "TIME",
                "time",
                Some(COL_WIDTH_TIME),
            ));
            column_view.append_column(&create_column_view_column_with_width(
                "LEVEL",
                "level",
                Some(COL_WIDTH_LEVEL),
            ));
            column_view.append_column(&create_column_view_column_with_width(
                "FUNCTION",
                "category",
                Some(COL_WIDTH_FUNCTION),
            ));
            column_view.append_column(&create_column_view_column("MESSAGE", "log"));
            // Expandable
        }
        logger::LogType::Message => {
            column_view.append_column(&create_column_view_column_with_width(
                "TIME",
                "time",
                Some(COL_WIDTH_TIME),
            ));
            column_view.append_column(&create_column_view_column_with_width(
                "SRC",
                "level",
                Some(COL_WIDTH_SRC),
            ));
            column_view.append_column(&create_column_view_column_with_width(
                "TYPE",
                "category",
                Some(COL_WIDTH_TYPE),
            ));
            column_view.append_column(&create_column_view_column("DETAILS", "log"));
            // Expandable
        }
        logger::LogType::Gst => {
            column_view.append_column(&create_column_view_column_with_width(
                "TIME",
                "time",
                Some(COL_WIDTH_TIME),
            ));
            column_view.append_column(&create_column_view_column_with_width(
                "LEVEL",
                "level",
                Some(COL_WIDTH_LEVEL),
            ));
            column_view.append_column(&create_column_view_column_with_width(
                "CATEGORY",
                "category",
                Some(COL_WIDTH_CATEGORY),
            ));
            column_view.append_column(&create_column_view_column_with_width(
                "FILE",
                "file",
                Some(COL_WIDTH_FILE),
            ));
            column_view.append_column(&create_column_view_column("LOG", "log"));
            // Expandable
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
                logger::LogType::Message => {
                    // Message format: time\tsrc\tmessage_type\tdetails
                    // Note: LogEntry fields are reused for different purposes:
                    //   - level field stores: source element name
                    //   - category field stores: message type
                    //   - log field stores: message details
                    let log: Vec<&str> = log_entry.splitn(4, '\t').collect();
                    LogEntry::new(
                        log.first().unwrap_or(&""), // time
                        log.get(1).unwrap_or(&""),  // level -> src
                        log.get(2).unwrap_or(&""),  // category -> message_type
                        "",                         // file (unused)
                        log.get(3).unwrap_or(&""),  // log -> details
                    )
                }
                logger::LogType::App => {
                    // App format from simplelog: "TIME LEVEL function\tmessage" or "TIME LEVEL message"
                    // Note: LogEntry fields are reused:
                    //   - category field stores: function name
                    //   - log field stores: log message
                    // Split by space first to get time and level
                    let parts: Vec<&str> = log_entry.splitn(3, ' ').collect();
                    let time = parts.first().unwrap_or(&"");
                    let level = parts.get(1).unwrap_or(&"");
                    let rest = parts.get(2).unwrap_or(&"");

                    // Check if rest contains a tab (GPS_* macros format)
                    let (function, message) = if rest.contains('\t') {
                        // Split by tab to get function and message
                        let func_msg: Vec<&str> = rest.splitn(2, '\t').collect();
                        (
                            *func_msg.first().unwrap_or(&""),
                            *func_msg.get(1).unwrap_or(&""),
                        )
                    } else {
                        // No tab - raw log macro used, no function name
                        ("", *rest)
                    };

                    // Clean up function name by removing redundant prefixes
                    let mut function_clean = function;

                    // Remove thread ID prefix like "(1) "
                    if let Some(pos) = function_clean.find(") ") {
                        if function_clean.starts_with('(') {
                            function_clean = &function_clean[pos + 2..];
                        }
                    }

                    // Remove redundant "gst_pipeline_studio" prefixes in order of specificity
                    // Try the longest prefix first to avoid double-stripping
                    let prefixes = [
                        "gst_pipeline_studio::logger: gst_pipeline_studio::",
                        "gst_pipeline_studio::logger::gst_pipeline_studio::",
                        "gst_pipeline_studio::",
                    ];

                    for prefix in &prefixes {
                        if let Some(stripped) = function_clean.strip_prefix(prefix) {
                            function_clean = stripped;
                            break; // Only strip once
                        }
                    }

                    // Clean up closure syntax: {{closure}} -> [closure]
                    let function_clean = function_clean.replace("{{closure}}", "[closure]");

                    LogEntry::new(
                        time,            // time
                        level,           // level
                        &function_clean, // category -> function
                        "",              // file (unused)
                        message,         // log -> message
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
