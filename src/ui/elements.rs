// elements.rs
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
use crate::settings::Settings;
use crate::ui::treeview;
use crate::GPS_DEBUG;
use gtk::prelude::*;
use gtk::{gdk::BUTTON_SECONDARY, Box, Label, ListStore, TreeView};
use gtk::{gio, glib};

pub fn reset_favorite_list(favorite_list: &TreeView) {
    let model = ListStore::new(&[String::static_type()]);
    favorite_list.set_model(Some(&model));
    let favorites = Settings::get_favorites_list();
    for favorite in favorites {
        model.insert_with_values(None, &[(0, &favorite)]);
    }
}

pub fn setup_favorite_list(app: &GPSApp) {
    let favorite_list: TreeView = app
        .builder
        .object("treeview-favorites")
        .expect("Couldn't get treeview-favorites");
    treeview::add_column_to_treeview(app, "treeview-favorites", "Name", 0);
    reset_favorite_list(&favorite_list);
    let app_weak = app.downgrade();
    favorite_list.connect_row_activated(move |tree_view, _tree_path, _tree_column| {
        let app = upgrade_weak!(app_weak);
        let selection = tree_view.selection();
        if let Some((model, iter)) = selection.selected() {
            let element_name = model.get::<String>(&iter, 0);
            GPS_DEBUG!("{} selected", element_name);
            app.add_new_element(&element_name);
        }
    });
    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);
    let app_weak = app.downgrade();
    gesture.connect_pressed(
        glib::clone!(@weak favorite_list => move |gesture, _n_press, x, y| {
            let app = upgrade_weak!(app_weak);
            if gesture.current_button() == BUTTON_SECONDARY {
                let selection = favorite_list.selection();
                if let Some((model, iter)) = selection.selected() {
                    let element_name = model
                    .get::<String>(&iter, 0);
                    GPS_DEBUG!("Element {} selected", element_name);

                    let pop_menu = app.app_pop_menu_at_position(&favorite_list, x, y);
                    let menu: gio::MenuModel = app
                    .builder
                    .object("fav_menu")
                    .expect("Couldn't get fav_menu model");
                    pop_menu.set_menu_model(Some(&menu));

                    app.connect_app_menu_action("favorite.remove",
                        move |_,_| {
                            Settings::remove_favorite(&element_name);
                            reset_favorite_list(&favorite_list);
                        }
                    );

                    pop_menu.show();
                }

            }
        }),
    );
    favorite_list.add_controller(&gesture);
}

pub fn add_to_favorite_list(app: &GPSApp, element_name: String) {
    let favorites = Settings::get_favorites_list();
    if !favorites.contains(&element_name) {
        let favorite_list: TreeView = app
            .builder
            .object("treeview-favorites")
            .expect("Couldn't get treeview-favorites");
        if let Some(model) = favorite_list.model() {
            let list_store = model
                .dynamic_cast::<ListStore>()
                .expect("Could not cast to ListStore");
            list_store.insert_with_values(None, &[(0, &element_name)]);
            Settings::add_favorite(&element_name);
        }
    }
}

fn reset_elements_list(elements_list: &TreeView) {
    let model = ListStore::new(&[String::static_type()]);
    elements_list.set_model(Some(&model));
    let elements = GPS::ElementInfo::elements_list().expect("Unable to obtain element's list");
    for element in elements {
        model.insert_with_values(None, &[(0, &element.name)]);
    }
}

pub fn setup_elements_list(app: &GPSApp) {
    let tree: TreeView = app
        .builder
        .object("treeview-elements")
        .expect("Couldn't get treeview-elements");
    treeview::add_column_to_treeview(app, "treeview-elements", "Name", 0);
    reset_elements_list(&tree);
    let app_weak = app.downgrade();
    tree.connect_row_activated(move |tree_view, _tree_path, _tree_column| {
        let app = upgrade_weak!(app_weak);
        let selection = tree_view.selection();
        if let Some((model, iter)) = selection.selected() {
            let element_name = model.get::<String>(&iter, 0);
            GPS_DEBUG!("{} selected", element_name);
            app.add_new_element(&element_name);
        }
    });
    let app_weak = app.downgrade();
    tree.connect_cursor_changed(move |tree_view| {
        let app = upgrade_weak!(app_weak);
        let selection = tree_view.selection();
        if let Some((model, iter)) = selection.selected() {
            let element_name = model.get::<String>(&iter, 0);
            let description = GPS::ElementInfo::element_description(&element_name)
                .expect("Unable to get element description from GStreamer");
            let box_property: Box = app
                .builder
                .object("box-property")
                .expect("Couldn't get treeview-elements");

            while let Some(child) = box_property.first_child() {
                box_property.remove(&child);
            }
            let label = Label::new(Some(""));
            label.set_hexpand(true);
            label.set_halign(gtk::Align::Start);
            label.set_margin_start(4);
            label.set_markup(&description);
            label.set_selectable(true);
            box_property.append(&label);
        }
    });
}
