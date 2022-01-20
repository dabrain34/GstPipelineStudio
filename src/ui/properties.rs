// properties.rs
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
use crate::gps as GPS;
use crate::logger;
use crate::GPS_TRACE;
use gtk::glib;
use gtk::prelude::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk::{Box, Button, Dialog, Entry, Label};

pub fn display_plugin_properties(app: &GPSApp, element_name: &str, node_id: u32) {
    let dialog: Dialog = app
        .builder
        .object("dialog-plugin-properties")
        .expect("Couldn't get dialog-plugin-properties");

    dialog.set_title(Some(&format!("{} properties", element_name)));
    dialog.set_default_size(640, 480);
    dialog.set_modal(true);

    let properties_box: Box = app
        .builder
        .object("box-plugin-properties")
        .expect("Couldn't get box-plugin-properties");
    let update_properties: Rc<RefCell<HashMap<String, String>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let properties = GPS::ElementInfo::element_properties(element_name).unwrap();
    for (name, value) in properties {
        let entry_box = Box::new(gtk::Orientation::Horizontal, 6);
        let label = Label::new(Some(&name));
        label.set_hexpand(true);
        label.set_halign(gtk::Align::Start);
        label.set_margin_start(4);
        entry_box.append(&label);
        let entry: Entry = Entry::new();
        entry.set_text(&value);
        entry.set_hexpand(true);
        entry.set_halign(gtk::Align::Start);
        entry.set_widget_name(&name);
        entry.connect_changed(
            glib::clone!(@weak entry, @strong update_properties => move |_| {
                GPS_TRACE!("property changed: {}:{}", entry.widget_name(), entry.text());
                update_properties.borrow_mut().insert(entry.widget_name().to_string(), entry.text().to_string());
            }),
        );
        entry_box.append(&entry);
        properties_box.append(&entry_box);
    }
    let properties_apply_btn: Button = app
        .builder
        .object("button-apply-plugin-properties")
        .expect("Couldn't get button-apply-plugin-properties");

    let app_weak = app.downgrade();
    properties_apply_btn.connect_clicked(
        glib::clone!(@strong update_properties, @weak dialog => move |_| {
            let app = upgrade_weak!(app_weak);
            app.update_element_properties(node_id, &update_properties.borrow());
            dialog.close();
        }),
    );

    dialog.show();
    for p in update_properties.borrow().values() {
        GPS_TRACE!("updated properties {}", p);
    }
}
