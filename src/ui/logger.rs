// logger.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::GPSApp;
use crate::ui::treeview;
use gtk::prelude::*;
use gtk::{gio, glib};

use gtk::{ListStore, TreeView};

fn reset_logger_list(logger_list: &TreeView) {
    let model = ListStore::new(&[
        String::static_type(),
        String::static_type(),
        String::static_type(),
    ]);
    logger_list.set_model(Some(&model));
}

pub fn setup_logger_list(app: &GPSApp) {
    treeview::add_column_to_treeview(app, "treeview-logger", "TIME", 0, false);
    treeview::add_column_to_treeview(app, "treeview-logger", "LEVEL", 1, false);
    treeview::add_column_to_treeview(app, "treeview-logger", "LOG", 2, true);
    let logger_list: TreeView = app
        .builder
        .object("treeview-logger")
        .expect("Couldn't get treeview-logger");
    reset_logger_list(&logger_list);

    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);
    let app_weak = app.downgrade();
    gesture.connect_pressed(
        glib::clone!(@weak logger_list => move |gesture, _n_press, x, y| {
            let app = upgrade_weak!(app_weak);
            if gesture.current_button() == gtk::gdk::BUTTON_SECONDARY {
                    let pop_menu = app.app_pop_menu_at_position(&logger_list, x, y);
                    let menu: gio::MenuModel = app
                    .builder
                    .object("logger_menu")
                    .expect("Couldn't get fav_menu model");
                    pop_menu.set_menu_model(Some(&menu));

                    app.connect_app_menu_action("logger.clear",
                        move |_,_| {
                            reset_logger_list(&logger_list);
                        }
                    );

                    pop_menu.show();
            }
        }),
    );
    logger_list.add_controller(gesture);
}

pub fn add_to_logger_list(app: &GPSApp, log_entry: &str) {
    let logger_list: TreeView = app
        .builder
        .object("treeview-logger")
        .expect("Couldn't get treeview-logger");
    if let Some(model) = logger_list.model() {
        let list_store = model
            .dynamic_cast::<ListStore>()
            .expect("Could not cast to ListStore");
        let log: Vec<&str> = log_entry.splitn(3, ' ').collect();
        list_store.insert_with_values(Some(0), &[(0, &log[0]), (1, &log[1]), (2, &log[2])]);
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
