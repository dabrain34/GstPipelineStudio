// treeview.rs
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
use gtk::prelude::TreeViewExt;
use gtk::{CellRendererText, TreeView, TreeViewColumn};

pub fn add_column_to_treeview(app: &GPSApp, tree_name: &str, column_name: &str, column_n: i32) {
    let treeview: TreeView = app
        .builder
        .object(tree_name)
        .expect("Couldn't get tree_name");
    let column = TreeViewColumn::new();
    let cell = CellRendererText::new();
    column.pack_start(&cell, true);
    // Association of the view's column with the model's `id` column.
    column.add_attribute(&cell, "text", column_n);
    column.set_title(column_name);
    treeview.append_column(&column);
}
