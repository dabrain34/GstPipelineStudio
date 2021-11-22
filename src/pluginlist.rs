// pluginlist.rs
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
use crate::graph::Element;
use crate::pipeline::ElementInfo;
use crate::pipeline::Pipeline;
use gtk::TextBuffer;
use gtk::{
    glib::{self, clone},
    prelude::*,
};

use gtk::{
    CellRendererText, Dialog, ListStore, TextView, TreeView, TreeViewColumn, WindowPosition,
};

fn create_and_fill_model(elements: &Vec<ElementInfo>) -> ListStore {
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

pub fn display_plugin_list(app: &GPSApp, elements: &Vec<ElementInfo>) {
    let dialog: Dialog = app
        .builder
        .object("dialog-plugin-list")
        .expect("Couldn't get window");

    dialog.set_title("Plugin list");
    dialog.set_position(WindowPosition::Center);
    dialog.set_default_size(640, 480);

    let tree: TreeView = app
        .builder
        .object("treeview-plugin-list")
        .expect("Couldn't get window");

    let text_view: TextView = app
        .builder
        .object("textview-plugin-list")
        .expect("Couldn't get window");
    let text_buffer: TextBuffer = text_view
        .buffer()
        .expect("Couldn't get buffer from text_view");
    if tree.n_columns() < 2 {
        append_column(&tree, 0);
        append_column(&tree, 1);
    }
    let model = create_and_fill_model(elements);
    // Setting the model into the view.
    tree.set_model(Some(&model));

    // The closure responds to selection changes by connection to "::cursor-changed" signal,
    // that gets emitted when the cursor moves (focus changes).
    tree.connect_cursor_changed(clone!(@weak dialog, @weak text_buffer => move |tree_view| {
        let selection = tree_view.selection();
        if let Some((model, iter)) = selection.selected() {
            let element_name = model
            .value(&iter, 1)
            .get::<String>()
            .expect("Treeview selection, column 1");
            let description = Pipeline::element_description(&element_name).expect("Unable to get element list from GStreamer");
            text_buffer.set_text("");
            text_buffer.insert_markup(&mut text_buffer.end_iter(), &description);
        }

    }));
    let app_weak = app.downgrade();
    tree.connect_row_activated(
        clone!(@weak dialog => move |tree_view, _tree_path, _tree_column| {
            let app = upgrade_weak!(app_weak);
            let selection = tree_view.selection();
            if let Some((model, iter)) = selection.selected() {
                // Now getting back the values from the row corresponding to the
                // iterator `iter`.
                //
                let element = Element {
                    name: model
                    .value(&iter, 1)
                    .get::<String>()
                    .expect("Treeview selection, column 1"),
                    position: (100.0,100.0),
                    size: (100.0,100.0),
                };

                let element_name = model
                .value(&iter, 1)
                .get::<String>()
                .expect("Treeview selection, column 1");
                app.add_new_element(element);

                println!("{}", element_name);
            }
        }),
    );

    dialog.connect_delete_event(|dialog, _| {
        dialog.hide();
        gtk::Inhibit(true)
    });
    dialog.show_all();
}
