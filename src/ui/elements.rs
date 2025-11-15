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
use crate::ui::models::ElementInfoObject;
use crate::GPS_DEBUG;
use gtk::prelude::*;
use gtk::{gdk::BUTTON_SECONDARY, Box, Label, SearchEntry};
use gtk::{gio, glib};
use gtk::{ColumnView, ColumnViewColumn, FilterListModel, SignalListItemFactory, SingleSelection};

// Helper function to create a column for ColumnView
fn create_column_view_column(title: &str, property: &str) -> ColumnViewColumn {
    let factory = SignalListItemFactory::new();
    let property_name = property.to_string();
    let property_name_clone = property_name.clone();

    factory.connect_setup(move |_, list_item| {
        let label = gtk::Label::new(None);
        label.set_halign(gtk::Align::Start);
        label.set_margin_start(4);
        label.set_margin_end(4);
        list_item
            .downcast_ref::<gtk::ListItem>()
            .expect("Needs to be ListItem")
            .set_child(Some(&label));
    });

    factory.connect_bind(move |_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk::ListItem>()
            .expect("Needs to be ListItem");
        let element_info = list_item
            .item()
            .and_downcast::<ElementInfoObject>()
            .expect("The item has to be an ElementInfoObject");
        let label = list_item
            .child()
            .and_downcast::<gtk::Label>()
            .expect("The child has to be a Label");

        let text = element_info.property::<String>(&property_name_clone);
        label.set_text(&text);
    });

    ColumnViewColumn::new(Some(title), Some(factory))
}

fn setup_search_entry(column_view: &ColumnView, app: &GPSApp) {
    let search_entry: SearchEntry = app
        .builder
        .object("elements-search-entry")
        .expect("Couldn't get elements-search-entry");

    // Get the filter model from the column view
    if let Some(selection_model) = column_view.model() {
        if let Some(selection) = selection_model.downcast_ref::<SingleSelection>() {
            if let Some(filter_model) = selection.model() {
                if let Some(filter) = filter_model.downcast_ref::<FilterListModel>() {
                    search_entry.connect_changed(glib::clone!(
                        #[weak]
                        filter,
                        move |entry| {
                            let entry_text = entry.text().to_string();

                            // Update the filter
                            if entry_text.is_empty() {
                                filter.set_filter(None::<&gtk::Filter>);
                            } else {
                                let custom_filter = gtk::CustomFilter::new(move |obj| {
                                    if let Some(element_info) =
                                        obj.downcast_ref::<ElementInfoObject>()
                                    {
                                        let name = element_info.property::<String>("name");
                                        name.contains(&entry_text)
                                    } else {
                                        false
                                    }
                                });
                                filter.set_filter(Some(&custom_filter));
                            }
                        }
                    ));
                }
            }
        }
    }
}

pub fn setup_favorite_list(app: &GPSApp) {
    let favorite_list: ColumnView = app
        .builder
        .object("treeview-favorites")
        .expect("Couldn't get treeview-favorites");

    // Add columns
    favorite_list.append_column(&create_column_view_column("Name", "name"));
    favorite_list.append_column(&create_column_view_column("Plugin", "plugin"));
    favorite_list.append_column(&create_column_view_column("Rank", "rank"));

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
    favorite_list.connect_activate(move |column_view, position| {
        let app = upgrade_weak!(app_weak);
        if let Some(model) = column_view.model() {
            if let Some(element_info) = model.item(position) {
                if let Some(element_info) = element_info.downcast_ref::<ElementInfoObject>() {
                    let element_name = element_info.property::<String>("name");
                    GPS_DEBUG!("{} selected", element_name);
                    app.add_new_element(&element_name);
                }
            }
        }
    });

    // Handle selection changes for property display
    if let Some(selection_model) = favorite_list.model() {
        if let Some(selection) = selection_model.downcast_ref::<SingleSelection>() {
            let app_weak = app.downgrade();
            selection.connect_selected_notify(move |selection| {
                let app = upgrade_weak!(app_weak);
                if let Some(element_info) = selection.selected_item() {
                    if let Some(element_info) = element_info.downcast_ref::<ElementInfoObject>() {
                        let element_name = element_info.property::<String>("name");
                        display_properties(&app, &element_name);
                    }
                }
            });
        }
    }

    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);
    let app_weak = app.downgrade();
    gesture.connect_pressed(glib::clone!(
        #[weak]
        favorite_list,
        move |gesture, _n_press, x, y| {
            let app = upgrade_weak!(app_weak);
            if gesture.current_button() == BUTTON_SECONDARY {
                if let Some(model) = favorite_list.model() {
                    if let Some(selection) = model.downcast_ref::<SingleSelection>() {
                        if let Some(element_info) = selection.selected_item() {
                            if let Some(element_info) =
                                element_info.downcast_ref::<ElementInfoObject>()
                            {
                                let element_name = element_info.property::<String>("name");
                                GPS_DEBUG!("Element {} selected", element_name);

                                let menu: gio::MenuModel = app
                                    .builder
                                    .object("fav_menu")
                                    .expect("Couldn't get fav_menu model");

                                let favorite_list_clone = favorite_list.clone();
                                app.connect_app_menu_action("favorite.remove", move |_, _| {
                                    Settings::remove_favorite(&element_name);
                                    reset_elements_list(
                                        &favorite_list_clone,
                                        get_favorite_elements(),
                                    );
                                });

                                app.show_context_menu_at_position(&favorite_list, x, y, &menu);
                            }
                        }
                    }
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
        let favorite_list: ColumnView = app
            .builder
            .object("treeview-favorites")
            .expect("Couldn't get treeview-favorites");
        if let Some(model) = favorite_list.model() {
            if let Some(selection) = model.downcast_ref::<SingleSelection>() {
                if let Some(filter_model) = selection.model() {
                    if let Some(filter) = filter_model.downcast_ref::<FilterListModel>() {
                        if let Some(list_store) = filter.model() {
                            if let Some(list_store) = list_store.downcast_ref::<gio::ListStore>() {
                                // Find the element info from the global list
                                let elements = GPS::ElementInfo::elements_list()
                                    .expect("Unable to obtain element's list");
                                if let Some(element) =
                                    elements.iter().find(|e| e.name == element_name)
                                {
                                    let element_info = ElementInfoObject::new(
                                        &element.name,
                                        &element.plugin_name,
                                        &element.rank.to_string(),
                                    );
                                    list_store.append(&element_info);
                                    Settings::add_favorite(&element_name);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn reset_elements_list(elements_list: &ColumnView, elements: Vec<GPS::ElementInfo>) {
    let model = gio::ListStore::new::<ElementInfoObject>();

    for element in elements {
        let element_info = ElementInfoObject::new(
            &element.name,
            &element.plugin_name,
            &element.rank.to_string(),
        );
        model.append(&element_info);
    }

    let filter_model = FilterListModel::new(Some(model), Option::<gtk::CustomFilter>::None);
    let selection_model = SingleSelection::new(Some(filter_model));
    elements_list.set_model(Some(&selection_model));
}

pub fn setup_elements_list(app: &GPSApp) {
    let tree: ColumnView = app
        .builder
        .object("treeview-elements")
        .expect("Couldn't get treeview-elements");

    // Add columns
    tree.append_column(&create_column_view_column("Name", "name"));
    tree.append_column(&create_column_view_column("Plugin", "plugin"));
    tree.append_column(&create_column_view_column("Rank", "rank"));

    let elements = GPS::ElementInfo::elements_list().expect("Unable to obtain element's list");
    reset_elements_list(&tree, elements);

    let app_weak = app.downgrade();
    tree.connect_activate(move |column_view, position| {
        let app = upgrade_weak!(app_weak);
        if let Some(model) = column_view.model() {
            if let Some(element_info) = model.item(position) {
                if let Some(element_info) = element_info.downcast_ref::<ElementInfoObject>() {
                    let element_name = element_info.property::<String>("name");
                    GPS_DEBUG!("{} selected", element_name);
                    app.add_new_element(&element_name);
                }
            }
        }
    });

    // Handle selection changes for property display
    if let Some(selection_model) = tree.model() {
        if let Some(selection) = selection_model.downcast_ref::<SingleSelection>() {
            let app_weak = app.downgrade();
            selection.connect_selected_notify(move |selection| {
                let app = upgrade_weak!(app_weak);
                if let Some(element_info) = selection.selected_item() {
                    if let Some(element_info) = element_info.downcast_ref::<ElementInfoObject>() {
                        let element_name = element_info.property::<String>("name");
                        display_properties(&app, &element_name);
                    }
                }
            });
        }
    }

    // Add right-click context menu for adding to favorites
    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);
    let app_weak = app.downgrade();
    gesture.connect_pressed(glib::clone!(
        #[weak]
        tree,
        move |gesture, _n_press, x, y| {
            let app = upgrade_weak!(app_weak);
            if gesture.current_button() == BUTTON_SECONDARY {
                if let Some(model) = tree.model() {
                    if let Some(selection) = model.downcast_ref::<SingleSelection>() {
                        if let Some(element_info) = selection.selected_item() {
                            if let Some(element_info) =
                                element_info.downcast_ref::<ElementInfoObject>()
                            {
                                let element_name = element_info.property::<String>("name");
                                GPS_DEBUG!("Element {} right-clicked", element_name);

                                let menu: gio::MenuModel = app
                                    .builder
                                    .object("elements_menu")
                                    .expect("Couldn't get elements_menu model");

                                let app_clone = app.clone();
                                app.connect_app_menu_action(
                                    "element.add-to-favorite",
                                    move |_, _| {
                                        add_to_favorite_list(&app_clone, element_name.clone());
                                    },
                                );

                                app.show_context_menu_at_position(&tree, x, y, &menu);
                            }
                        }
                    }
                }
            }
        }
    ));
    tree.add_controller(gesture);

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
