// plugindialogs.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
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
use crate::logger;
use crate::pipeline::ElementInfo;
use crate::pipeline::Pipeline;
use gtk::glib;
use gtk::prelude::*;
use gtk::TextBuffer;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk::{
    Box, Button, CellRendererText, Dialog, Entry, Label, ListStore, TextView, TreeView,
    TreeViewColumn,
};

fn create_and_fill_model(elements: &[ElementInfo]) -> ListStore {
    // Creation of a model with two rows.
    let model = ListStore::new(&[u32::static_type(), String::static_type()]);

    // Filling up the tree view.
    for (i, entry) in elements.iter().enumerate() {
        model.insert_with_values(
            None,
            &[(0, &(i as u32 + 1)), (1, &entry.name.as_ref().unwrap())],
        );
    }

    model
}

fn append_column(tree: &TreeView, id: i32) {
    let column = TreeViewColumn::new();
    let cell = CellRendererText::new();

    column.pack_start(&cell, true);
    // Association of the view's column with the model's `id` column.
    column.add_attribute(&cell, "text", id);
    tree.append_column(&column);
}

pub fn display_plugin_list(app: &GPSApp, elements: &[ElementInfo]) {
    let dialog: Dialog = app
        .builder
        .object("dialog-plugin-list")
        .expect("Couldn't get the dialog-plugin-list window");

    if app.plugin_list_initialized.get().is_none() {
        dialog.set_title(Some("Plugin list"));
        dialog.set_default_size(640, 480);

        let text_view: TextView = app
            .builder
            .object("textview-plugin-list")
            .expect("Couldn't get textview-plugin-list window");
        let text_buffer: TextBuffer = text_view.buffer();

        let tree: TreeView = app
            .builder
            .object("treeview-plugin-list")
            .expect("Couldn't get treeview-plugin-list window");
        if tree.n_columns() < 2 {
            append_column(&tree, 0);
            append_column(&tree, 1);
        }
        tree.set_search_column(1);
        let model = create_and_fill_model(elements);
        // Setting the model into the view.
        tree.set_model(Some(&model));

        // The closure responds to selection changes by connection to "::cursor-changed" signal,
        // that gets emitted when the cursor moves (focus changes).
        tree.connect_cursor_changed(glib::clone!(@weak dialog, @weak text_buffer => move |tree_view| {
            let selection = tree_view.selection();
            if let Some((model, iter)) = selection.selected() {
                let element_name = model
                .get(&iter, 1)
                .get::<String>()
                .expect("Unable to get the treeview selection, column 1");
                let description = Pipeline::element_description(&element_name).expect("Unable to get element description from GStreamer");
                text_buffer.set_text("");
                text_buffer.insert_markup(&mut text_buffer.end_iter(), &description);
            }

        }));
        let app_weak = app.downgrade();
        tree.connect_row_activated(
            glib::clone!(@weak dialog => move |tree_view, _tree_path, _tree_column| {
                let app = upgrade_weak!(app_weak);
                let selection = tree_view.selection();
                if let Some((model, iter)) = selection.selected() {
                    let element_name = model
                    .get(&iter, 1)
                    .get::<String>()
                    .expect("Unable to get the treeview selection, column 1");
                    app.add_new_element(&element_name);
                }
            }),
        );
        app.plugin_list_initialized.set(true).unwrap();
    }

    dialog.show();
}

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
    let properties = Pipeline::element_properties(element_name).unwrap();
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
                GPS_LOG!("{}:{}", entry.widget_name(), entry.text());
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
        GPS_LOG!("updated properties {}", p);
    }
}
