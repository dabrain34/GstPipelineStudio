// logger.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::GPSApp;
use crate::logger;
use crate::ui::treeview;
use gtk::prelude::*;
use gtk::{gio, glib};

use gtk::{ListStore, TreeView};

fn reset_logger_list(logger_list: &TreeView) {
    let model = ListStore::new(&[
        String::static_type(),
        String::static_type(),
        String::static_type(),
        String::static_type(),
        String::static_type(),
    ]);
    logger_list.set_model(Some(&model));
}

pub fn setup_logger_list(app: &GPSApp, logger_name: &str, log_type: logger::LogType) {
    match log_type {
        logger::LogType::App => {
            treeview::add_column_to_treeview(app, logger_name, "TIME", 0, false);
            treeview::add_column_to_treeview(app, logger_name, "LEVEL", 1, false);
            treeview::add_column_to_treeview(app, logger_name, "LOG", 2, true);
        }
        logger::LogType::Gst => {
            treeview::add_column_to_treeview(app, logger_name, "TIME", 0, false);
            treeview::add_column_to_treeview(app, logger_name, "LEVEL", 1, false);
            treeview::add_column_to_treeview(app, logger_name, "CATEGORY", 2, false);
            treeview::add_column_to_treeview(app, logger_name, "FILE", 3, false);
            treeview::add_column_to_treeview(app, logger_name, "LOG", 4, true);
        }
        logger::LogType::Message => {
            treeview::add_column_to_treeview(app, logger_name, "TIME", 0, false);
            treeview::add_column_to_treeview(app, logger_name, "LEVEL", 1, false);
            treeview::add_column_to_treeview(app, logger_name, "LOG", 2, true);
        }
    }

    let logger_list: TreeView = app
        .builder
        .object(logger_name)
        .expect("Couldn't get treeview-app-logger");
    reset_logger_list(&logger_list);

    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);
    let app_weak = app.downgrade();
    gesture.connect_pressed(glib::clone!(
        #[weak]
        logger_list,
        move |gesture, _n_press, x, y| {
            let app = upgrade_weak!(app_weak);
            if gesture.current_button() == gtk::gdk::BUTTON_SECONDARY {
                let pop_menu = app.app_pop_menu_at_position(&logger_list, x, y);
                let menu: gio::MenuModel = app
                    .builder
                    .object("logger_menu")
                    .expect("Couldn't get fav_menu model");
                pop_menu.set_menu_model(Some(&menu));

                app.connect_app_menu_action("logger.clear", move |_, _| {
                    reset_logger_list(&logger_list);
                });

                pop_menu.show();
            }
        }
    ));
    logger_list.add_controller(gesture);
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
    let logger_list: TreeView = app
        .builder
        .object(log_tree_name.as_str())
        .expect("Couldn't get treeview");
    if let Some(model) = logger_list.model() {
        let list_store = model
            .dynamic_cast::<ListStore>()
            .expect("Could not cast to ListStore");
        if log_type == logger::LogType::Gst {
            let log: Vec<&str> = log_entry.splitn(5, '\t').collect();
            list_store.insert_with_values(
                Some(0),
                &[
                    (0, &log[0]),
                    (1, &log[1]),
                    (2, &log[2]),
                    (3, &log[3]),
                    (4, &log[4]),
                ],
            );
        } else {
            let log: Vec<&str> = log_entry.splitn(3, ' ').collect();
            let mut indexed_vec: Vec<(u32, &dyn ToValue)> = Vec::new();

            for (index, item) in log.iter().enumerate() {
                indexed_vec.push((index as u32, item));
            }
            list_store.insert_with_values(Some(0), &indexed_vec);
        }
        // Scroll to the first element.
        if let Some(model) = logger_list.model() {
            if let Some(iter) = model.iter_first() {
                let path = model.path(&iter);
                logger_list.scroll_to_cell(
                    Some(&path),
                    None::<&gtk::TreeViewColumn>,
                    false,
                    0.0,
                    0.0,
                );
            }
        }
    }
}
