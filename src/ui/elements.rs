// elements.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::GPSApp;
use crate::gps as GPS;
use crate::logger;
use crate::settings::Settings;
use crate::ui::treeview;
use crate::GPS_DEBUG;
use gtk::prelude::*;
use gtk::{gdk::BUTTON_SECONDARY, Box, Label, ListStore, SearchEntry, TreeModelFilter, TreeView};
use gtk::{gio, glib};

fn setup_search_entry(tree: &TreeView, app: &GPSApp) {
    tree.set_search_equal_func(|model, _col, key, data| {
        let entry_name = model.get::<String>(data, 0);
        !entry_name.contains(key)
    });

    let search_entry: SearchEntry = app
        .builder
        .object("elements-search-entry")
        .expect("Couldn't get elements-search-entry");
    tree.set_search_entry(Some(&search_entry));

    let model: TreeModelFilter = tree
        .model()
        .and_downcast()
        .expect("Could not find a TreeModelFilter");
    let model: ListStore = model
        .model()
        .downcast()
        .expect("TreeModelFilter does not contains a ListStore");

    search_entry.connect_changed(move |entry| {
        let entry_text = entry.text().to_string();

        let iter = match model.iter_first() {
            Some(iter) => iter,
            None => return,
        };

        loop {
            let element_name = model.get::<String>(&iter, 0);
            model.set_value(&iter, 3, &element_name.contains(&entry_text).to_value());

            if !model.iter_next(&iter) {
                break;
            }
        }
    });
}

pub fn setup_favorite_list(app: &GPSApp) {
    let favorite_list: TreeView = app
        .builder
        .object("treeview-favorites")
        .expect("Couldn't get treeview-favorites");

    treeview::add_column_to_treeview(app, "treeview-favorites", "Name", 0, false);
    treeview::add_column_to_treeview(app, "treeview-favorites", "Plugin", 1, false);
    treeview::add_column_to_treeview(app, "treeview-favorites", "Rank", 2, false);

    let get_favorite_elements = || -> Vec<GPS::ElementInfo> {
        let favorite_names = Settings::favorites_list();

        GPS::ElementInfo::elements_list()
            .expect("Unable to obtain element's list")
            .into_iter()
            .filter(|e| favorite_names.contains(&e.name))
            .collect()
    };

    reset_elements_list(&favorite_list, get_favorite_elements());

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
    let app_weak = app.downgrade();
    favorite_list.connect_cursor_changed(move |tree_view| {
        let app = upgrade_weak!(app_weak);
        let selection = tree_view.selection();
        if let Some((model, iter)) = selection.selected() {
            let element_name = model.get::<String>(&iter, 0);
            display_properties(&app, &element_name);
        }
    });
    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);
    let app_weak = app.downgrade();
    gesture.connect_pressed(glib::clone!(
        #[weak]
        favorite_list,
        move |gesture, _n_press, x, y| {
            let app = upgrade_weak!(app_weak);
            if gesture.current_button() == BUTTON_SECONDARY {
                let selection = favorite_list.selection();
                if let Some((model, iter)) = selection.selected() {
                    let element_name = model.get::<String>(&iter, 0);
                    GPS_DEBUG!("Element {} selected", element_name);

                    let menu: gio::MenuModel = app
                        .builder
                        .object("fav_menu")
                        .expect("Couldn't get fav_menu model");

                    let favorite_list_clone = favorite_list.clone();
                    app.connect_app_menu_action("favorite.remove", move |_, _| {
                        Settings::remove_favorite(&element_name);
                        reset_elements_list(&favorite_list_clone, get_favorite_elements());
                    });

                    app.show_context_menu_at_position(&favorite_list, x, y, &menu);
                }
            }
        }
    ));
    favorite_list.add_controller(gesture);

    setup_search_entry(&favorite_list, app)
}

pub fn add_to_favorite_list(app: &GPSApp, element_name: String) {
    let mut favorites = Settings::favorites_list();
    favorites.sort();
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

fn reset_elements_list(elements_list: &TreeView, elements: Vec<GPS::ElementInfo>) {
    let model = ListStore::new(&[
        String::static_type(),
        String::static_type(),
        String::static_type(),
        bool::static_type(),
    ]);
    elements_list.set_model(Some(&model));
    for element in elements {
        model.insert_with_values(
            None,
            &[
                (0, &element.name),
                (1, &element.plugin_name),
                (2, &element.rank.to_string()),
                (3, &true),
            ],
        );
    }

    let filter_model = TreeModelFilter::new(&model, None);
    filter_model.set_visible_column(3);

    elements_list.set_model(Some(&filter_model));
}

pub fn setup_elements_list(app: &GPSApp) {
    let tree: TreeView = app
        .builder
        .object("treeview-elements")
        .expect("Couldn't get treeview-elements");
    treeview::add_column_to_treeview(app, "treeview-elements", "Name", 0, false);
    treeview::add_column_to_treeview(app, "treeview-elements", "Plugin", 1, false);
    treeview::add_column_to_treeview(app, "treeview-elements", "Rank", 2, false);

    let elements = GPS::ElementInfo::elements_list().expect("Unable to obtain element's list");
    reset_elements_list(&tree, elements);

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
            display_properties(&app, &element_name);
        }
    });

    setup_search_entry(&tree, app)
}

pub fn display_properties(app: &GPSApp, element_name: &str) {
    let description = GPS::ElementInfo::element_description(element_name)
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
