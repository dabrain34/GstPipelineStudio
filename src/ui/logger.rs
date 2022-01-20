// logger.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-only
use crate::app::GPSApp;
use crate::ui::treeview;
use gtk::prelude::*;

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
    treeview::add_column_to_treeview(app, "treeview-logger", "TIME", 0);
    treeview::add_column_to_treeview(app, "treeview-logger", "LEVEL", 1);
    treeview::add_column_to_treeview(app, "treeview-logger", "LOG", 2);
    let logger_list: TreeView = app
        .builder
        .object("treeview-logger")
        .expect("Couldn't get treeview-logger");
    reset_logger_list(&logger_list);
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
        list_store.insert_with_values(None, &[(0, &log[0]), (1, &log[1]), (2, &log[2])]);
    }
}
