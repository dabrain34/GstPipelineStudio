// properties.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::GPSApp;
use crate::common;
use crate::gps as GPS;
use crate::graphbook;
use crate::logger;
use crate::ui as GPSUI;
use crate::{GPS_INFO, GPS_TRACE};
use gtk::glib;
use gtk::prelude::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type PropertyVec = (
    Vec<(String, glib::ParamSpec)>,
    Vec<(String, glib::ParamSpec)>,
);

/// Common GStreamer properties that users typically need to modify.
/// These are shown in the "Basic" tab of the properties dialog.
const COMMON_PROPERTIES: &[&str] = &[
    "name",
    "async",
    "enable-last-sample",
    "qos",
    "sync",
    "location",
    "device",
    "width",
    "height",
    "framerate",
    "format",
    "uri",
    "volume",
    "mute",
];

/// Helper function to filter property rows based on search text.
/// Traverses the widget hierarchy to find property labels and show/hide rows accordingly.
fn filter_property_rows(container: &gtk::Box, search_text: &str) {
    let search_lower = search_text.to_lowercase();

    let mut child = container.first_child();
    while let Some(widget) = child {
        if let Some(category_box) = widget.downcast_ref::<gtk::Box>() {
            let mut listbox_child = category_box.first_child();
            while let Some(lb_widget) = listbox_child {
                if let Some(listbox) = lb_widget.downcast_ref::<gtk::ListBox>() {
                    let mut row_child = listbox.first_child();
                    while let Some(row_widget) = row_child {
                        if let Some(row_box) = row_widget.first_child() {
                            if let Some(row) = row_box.downcast_ref::<gtk::Box>() {
                                if let Some(label_widget) = row.first_child() {
                                    if let Some(label) = label_widget.downcast_ref::<gtk::Label>() {
                                        let label_text = label.text().to_lowercase();
                                        row_widget.set_visible(
                                            search_lower.is_empty()
                                                || label_text.contains(&search_lower),
                                        );
                                    }
                                }
                            }
                        }
                        row_child = row_widget.next_sibling();
                    }
                }
                listbox_child = lb_widget.next_sibling();
            }
        }
        child = widget.next_sibling();
    }
}

/// Helper function to filter pad property rows in a ListBox based on search text.
/// Searches property names within ListBoxRow widgets.
fn filter_pad_property_rows(listbox: &gtk::ListBox, search_text: &str) {
    let search_lower = search_text.to_lowercase();

    let mut row_child = listbox.first_child();
    while let Some(row_widget) = row_child {
        if let Some(listbox_row) = row_widget.downcast_ref::<gtk::ListBoxRow>() {
            if let Some(row_box) = listbox_row.child() {
                if let Some(content) = row_box.downcast_ref::<gtk::Box>() {
                    // For pad properties, the label is in a header_row (first child)
                    if let Some(header_widget) = content.first_child() {
                        if let Some(header_row) = header_widget.downcast_ref::<gtk::Box>() {
                            if let Some(label_widget) = header_row.first_child() {
                                if let Some(label) = label_widget.downcast_ref::<gtk::Label>() {
                                    let label_text = label.text().to_lowercase();
                                    listbox_row.set_visible(
                                        search_lower.is_empty()
                                            || label_text.contains(&search_lower),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        row_child = row_widget.next_sibling();
    }
}

pub fn property_to_widget<F: Fn(String, String) + 'static>(
    app: &GPSApp,
    node_id: u32,
    element_name: &str,
    property_name: &str,
    param: &glib::ParamSpec,
    f: F,
) -> Option<gtk::Widget> {
    match param.type_() {
        _t if param.type_() == glib::ParamSpecBoolean::static_type() => {
            let check_button = gtk::CheckButton::new();
            check_button.set_widget_name(property_name);
            GPS_TRACE!("add CheckBox property : {}", check_button.widget_name());
            if let Some(value) = app.element_property(node_id, property_name) {
                check_button.set_active(value.parse::<bool>().unwrap_or(false));
            } else if (param.flags() & glib::ParamFlags::READABLE) == glib::ParamFlags::READABLE
                || (param.flags() & glib::ParamFlags::READWRITE) == glib::ParamFlags::READWRITE
            {
                if let Ok(value) =
                    GPS::ElementInfo::element_property_by_feature_name(element_name, param.name())
                {
                    check_button.set_active(value.parse::<bool>().unwrap_or(false));
                }
            } else if let Some(value) = common::value_as_str(param.default_value()) {
                check_button.set_active(value.parse::<bool>().unwrap_or(false));
            }
            check_button.connect_toggled(glib::clone!(move |c| {
                f(c.widget_name().to_string(), c.is_active().to_string());
            }));
            Some(check_button.upcast::<gtk::Widget>())
        }
        t if [
            glib::ParamSpecInt::static_type(),
            glib::ParamSpecUInt::static_type(),
            glib::ParamSpecInt64::static_type(),
            glib::ParamSpecUInt64::static_type(),
            glib::ParamSpecString::static_type(),
            glib::ParamSpecFloat::static_type(),
        ]
        .contains(&t) =>
        {
            let entry = gtk::Entry::new();
            entry.set_width_request(350);
            entry.set_widget_name(property_name);
            GPS_TRACE!("Add Edit property : {}", entry.widget_name());
            if let Some(value) = app.element_property(node_id, property_name) {
                entry.set_text(&value);
            } else if (param.flags() & glib::ParamFlags::READABLE) == glib::ParamFlags::READABLE
                || (param.flags() & glib::ParamFlags::READWRITE) == glib::ParamFlags::READWRITE
            {
                if let Ok(value) =
                    GPS::ElementInfo::element_property_by_feature_name(element_name, param.name())
                {
                    entry.set_text(&value);
                }
            } else if let Some(value) = common::value_as_str(param.default_value()) {
                entry.set_text(&value);
            }

            entry.connect_changed(glib::clone!(move |e| {
                f(e.widget_name().to_string(), e.text().to_string())
            }));
            Some(entry.upcast::<gtk::Widget>())
        }
        t if [
            glib::ParamSpecEnum::static_type(),
            glib::ParamSpecFlags::static_type(),
        ]
        .contains(&t) =>
        {
            let string_list = gtk::StringList::new(&[]);

            // Add an empty entry to be able to reset the value
            string_list.append("");

            if t.is_a(glib::ParamSpecEnum::static_type()) {
                let param = param
                    .clone()
                    .downcast::<glib::ParamSpecEnum>()
                    .expect("Should be a ParamSpecEnum");
                let enums = param.enum_class();
                for value in enums.values() {
                    string_list.append(&format!(
                        "{}:{}:{}",
                        value.value(),
                        value.nick(),
                        value.name()
                    ));
                }
            } else if t.is_a(glib::ParamSpecFlags::static_type()) {
                let param = param
                    .clone()
                    .downcast::<glib::ParamSpecFlags>()
                    .expect("Should be a ParamSpecFlags");
                let flags = param.flags_class();
                for value in flags.values() {
                    string_list.append(&format!(
                        "{}:{}:{}",
                        value.value(),
                        value.nick(),
                        value.name()
                    ));
                }
            }

            let dropdown =
                gtk::DropDown::new(Some(string_list.clone()), Option::<gtk::Expression>::None);
            dropdown.set_widget_name(property_name);
            GPS_TRACE!("add DropDown property : {}", dropdown.widget_name());

            if let Some(value) = app.element_property(node_id, property_name) {
                //Retrieve the first value (index) from the property
                dropdown.set_selected(value.parse::<u32>().unwrap_or(0) + 1);
            } else if (param.flags() & glib::ParamFlags::READABLE) == glib::ParamFlags::READABLE
                || (param.flags() & glib::ParamFlags::READWRITE) == glib::ParamFlags::READWRITE
            {
                if let Ok(value) =
                    GPS::ElementInfo::element_property_by_feature_name(element_name, param.name())
                {
                    dropdown.set_selected(value.parse::<u32>().unwrap_or(0) + 1);
                }
            }

            dropdown.connect_selected_notify(move |d| {
                if let Some(selected_item) = d.selected_item() {
                    if let Some(string_object) = selected_item.downcast_ref::<gtk::StringObject>() {
                        let text = string_object.string();
                        let value = text.to_string();
                        let value = value.split_once(':');
                        f(
                            d.widget_name().to_string(),
                            value.unwrap_or_default().0.to_string(),
                        );
                    }
                }
            });
            Some(dropdown.upcast::<gtk::Widget>())
        }
        _ => {
            GPS_INFO!(
                "Property not supported : name={} type={}",
                property_name,
                param.type_()
            );
            None
        }
    }
}

fn create_property_category_ui(
    category_name: &str,
    properties: &[(&String, &glib::ParamSpec)],
    app: &GPSApp,
    node_id: u32,
    element_name: &str,
    update_properties: &Rc<RefCell<HashMap<String, String>>>,
) -> gtk::Box {
    let category_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    category_box.set_margin_start(12);
    category_box.set_margin_end(12);
    category_box.set_margin_top(8);
    category_box.set_margin_bottom(8);

    if !properties.is_empty() {
        // Category header
        let header = gtk::Label::builder()
            .label(category_name)
            .halign(gtk::Align::Start)
            .css_classes(vec!["heading"])
            .build();
        category_box.append(&header);

        // Properties list box for better visual grouping
        let listbox = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(vec!["boxed-list"])
            .build();

        for (name, param) in properties {
            let widget = property_to_widget(
                app,
                node_id,
                element_name,
                name,
                param,
                glib::clone!(
                    #[strong]
                    update_properties,
                    move |name, value| {
                        GPS_INFO!("property changed: {}:{}", name, value);
                        update_properties.borrow_mut().insert(name, value);
                    }
                ),
            );

            if let Some(widget) = widget {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                row.set_margin_start(12);
                row.set_margin_end(12);
                row.set_margin_top(8);
                row.set_margin_bottom(8);

                // Property name with better formatting
                let label = gtk::Label::builder()
                    .label(name.replace('-', " ").to_string())
                    .halign(gtk::Align::Start)
                    .hexpand(true)
                    .xalign(0.0)
                    .build();

                // Add tooltip with property description if available
                if let Some(blurb) = param.blurb() {
                    label.set_tooltip_text(Some(blurb));
                }

                row.append(&label);
                row.append(&widget);

                listbox.append(&row);
            }
        }

        category_box.append(&listbox);
    }

    category_box
}

fn categorize_properties(properties: &HashMap<String, glib::ParamSpec>) -> PropertyVec {
    let mut basic = Vec::new();
    let mut advanced = Vec::new();

    for (name, param) in properties {
        let is_common = COMMON_PROPERTIES.iter().any(|&prop| name == prop);

        if is_common {
            basic.push((name.clone(), param.clone()));
        } else {
            advanced.push((name.clone(), param.clone()));
        }
    }

    basic.sort_by(|a, b| a.0.cmp(&b.0));
    advanced.sort_by(|a, b| a.0.cmp(&b.0));

    (basic, advanced)
}

pub fn display_plugin_properties(app: &GPSApp, element_name: &str, node_id: u32) {
    let update_properties: Rc<RefCell<HashMap<String, String>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let properties = GPS::ElementInfo::element_properties_by_feature_name(element_name).unwrap();

    // Main container
    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Search bar
    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Search properties...")
        .margin_start(12)
        .margin_end(12)
        .margin_top(12)
        .margin_bottom(8)
        .build();

    main_box.append(&search_entry);

    // Categorize properties
    let (basic_props, advanced_props) = categorize_properties(&properties);

    // Create notebook for tabs
    let notebook = gtk::Notebook::new();
    notebook.set_margin_start(12);
    notebook.set_margin_end(12);
    notebook.set_margin_bottom(12);

    // All properties view (scrollable)
    let all_scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(400)
        .build();

    let all_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let mut all_properties: Vec<(&String, &glib::ParamSpec)> = properties.iter().collect();
    all_properties.sort_by(|a, b| a.0.cmp(b.0));

    // Group by category for "All" tab
    if !basic_props.is_empty() {
        let basic_refs: Vec<(&String, &glib::ParamSpec)> =
            basic_props.iter().map(|(n, p)| (n, p)).collect();
        all_box.append(&create_property_category_ui(
            "Common Properties",
            &basic_refs,
            app,
            node_id,
            element_name,
            &update_properties,
        ));
    }

    if !advanced_props.is_empty() {
        let advanced_refs: Vec<(&String, &glib::ParamSpec)> =
            advanced_props.iter().map(|(n, p)| (n, p)).collect();
        all_box.append(&create_property_category_ui(
            "Advanced Properties",
            &advanced_refs,
            app,
            node_id,
            element_name,
            &update_properties,
        ));
    }

    all_scrolled.set_child(Some(&all_box));

    // Basic properties view
    let basic_scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(400)
        .build();

    let basic_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    if !basic_props.is_empty() {
        let basic_refs: Vec<(&String, &glib::ParamSpec)> =
            basic_props.iter().map(|(n, p)| (n, p)).collect();
        basic_box.append(&create_property_category_ui(
            "Common Properties",
            &basic_refs,
            app,
            node_id,
            element_name,
            &update_properties,
        ));
    } else {
        let empty_label = gtk::Label::builder()
            .label("No common properties available")
            .margin_top(24)
            .css_classes(vec!["dim-label"])
            .build();
        basic_box.append(&empty_label);
    }
    basic_scrolled.set_child(Some(&basic_box));

    // Advanced properties view
    let advanced_scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(400)
        .build();

    let advanced_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    if !advanced_props.is_empty() {
        let advanced_refs: Vec<(&String, &glib::ParamSpec)> =
            advanced_props.iter().map(|(n, p)| (n, p)).collect();
        advanced_box.append(&create_property_category_ui(
            "Advanced Properties",
            &advanced_refs,
            app,
            node_id,
            element_name,
            &update_properties,
        ));
    }
    advanced_scrolled.set_child(Some(&advanced_box));

    // Add tabs
    notebook.append_page(&basic_scrolled, Some(&gtk::Label::new(Some("Basic"))));
    notebook.append_page(&advanced_scrolled, Some(&gtk::Label::new(Some("Advanced"))));
    notebook.append_page(&all_scrolled, Some(&gtk::Label::new(Some("All"))));

    main_box.append(&notebook);

    // Search functionality - filter all three tabs
    let all_box_weak = all_box.downgrade();
    let basic_box_weak = basic_box.downgrade();
    let advanced_box_weak = advanced_box.downgrade();

    search_entry.connect_search_changed(move |entry| {
        let search_text = entry.text().to_string();

        for box_weak in [&all_box_weak, &basic_box_weak, &advanced_box_weak] {
            if let Some(container) = box_weak.upgrade() {
                filter_property_rows(&container, &search_text);
            }
        }
    });

    // Convert Box to Grid for dialog compatibility
    let grid = gtk::Grid::new();
    grid.attach(&main_box, 0, 0, 1, 1);

    let dialog = GPSUI::dialog::create(
        &format!("{element_name} properties"),
        app,
        &grid,
        glib::clone!(
            #[strong]
            update_properties,
            move |app, _dialog| {
                app.update_element_properties(node_id, &update_properties.borrow());
            }
        ),
    );

    dialog.set_default_size(650, 550);
    dialog.present();
}

pub fn display_pad_properties(
    app: &GPSApp,
    element_name: &str,
    port_name: &str,
    node_id: u32,
    port_id: u32,
) {
    // Track property changes in two separate collections:
    // - update_properties: Contains new values for modified/added properties
    // - deleted_properties: Contains names of properties to be removed
    let update_properties: Rc<RefCell<HashMap<String, String>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let deleted_properties: Rc<RefCell<std::collections::HashSet<String>>> =
        Rc::new(RefCell::new(std::collections::HashSet::new()));

    // Main container
    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Search bar
    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Search pad properties...")
        .margin_start(12)
        .margin_end(12)
        .margin_top(12)
        .margin_bottom(8)
        .build();

    main_box.append(&search_entry);

    // Scrollable content area
    let scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(300)
        .margin_start(12)
        .margin_end(12)
        .margin_bottom(12)
        .build();

    let content_box = gtk::Box::new(gtk::Orientation::Vertical, 8);

    // Create listbox for existing properties
    let listbox = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(vec!["boxed-list"])
        .build();

    let properties = app.pad_properties(node_id, port_id);
    for (name, value) in properties {
        let name_for_change = name.clone();
        let name_for_remove = name.clone();

        let row_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);

        // Header row with property name and remove button
        let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);

        let property_name_label = gtk::Label::builder()
            .label(&name)
            .halign(gtk::Align::Start)
            .hexpand(true)
            .xalign(0.0)
            .build();
        header_row.append(&property_name_label);

        let remove_button = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove property")
            .halign(gtk::Align::End)
            .build();
        header_row.append(&remove_button);

        row_box.append(&header_row);

        let property_value = gtk::Entry::new();
        property_value.set_text(&value);
        property_value.set_hexpand(true);

        property_value.connect_changed(glib::clone!(
            #[strong]
            update_properties,
            move |entry| {
                update_properties
                    .borrow_mut()
                    .insert(name_for_change.clone(), entry.text().to_string());
            }
        ));

        row_box.append(&property_value);

        let list_row = gtk::ListBoxRow::new();
        list_row.set_child(Some(&row_box));
        list_row.set_activatable(false);

        // Connect remove button to remove the row and property
        remove_button.connect_clicked(glib::clone!(
            #[weak]
            list_row,
            #[weak]
            listbox,
            #[strong]
            update_properties,
            #[strong]
            deleted_properties,
            move |_| {
                // Track this property for deletion when Apply is clicked
                deleted_properties
                    .borrow_mut()
                    .insert(name_for_remove.clone());
                // Remove from pending updates (in case it was modified before deletion)
                update_properties.borrow_mut().remove(&name_for_remove);
                // Remove from UI immediately for visual feedback
                listbox.remove(&list_row);
            }
        ));

        listbox.append(&list_row);
    }

    content_box.append(&listbox);

    // Add new property section
    let new_prop_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    new_prop_box.set_margin_top(12);

    let header = gtk::Label::builder()
        .label("Add New Property")
        .halign(gtk::Align::Start)
        .css_classes(vec!["heading"])
        .build();
    new_prop_box.append(&header);

    let new_prop_listbox = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(vec!["boxed-list"])
        .build();

    let new_row_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    new_row_box.set_margin_start(12);
    new_row_box.set_margin_end(12);
    new_row_box.set_margin_top(8);
    new_row_box.set_margin_bottom(8);

    let name_label = gtk::Label::builder()
        .label("Property Name")
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .build();
    new_row_box.append(&name_label);

    let property_name = gtk::Entry::new();
    property_name.set_hexpand(true);
    new_row_box.append(&property_name);

    let value_label = gtk::Label::builder()
        .label("Property Value")
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .margin_top(8)
        .build();
    new_row_box.append(&value_label);

    let property_value = gtk::Entry::new();
    property_value.set_hexpand(true);
    new_row_box.append(&property_value);

    // Add button
    let add_button = gtk::Button::builder()
        .label("Add Property")
        .margin_top(12)
        .halign(gtk::Align::End)
        .build();

    add_button.connect_clicked(glib::clone!(
        #[weak]
        property_name,
        #[weak]
        property_value,
        #[weak(rename_to = parent_listbox)]
        listbox,
        #[strong]
        update_properties,
        #[strong]
        deleted_properties,
        move |_| {
            let name = property_name.text().to_string();
            let value = property_value.text().to_string();

            if name.is_empty() {
                return;
            }

            // Check for duplicate property names in existing properties
            let mut property_exists = false;
            let mut row_child = parent_listbox.first_child();
            while let Some(row_widget) = row_child {
                if let Some(listbox_row) = row_widget.downcast_ref::<gtk::ListBoxRow>() {
                    if let Some(row_box) = listbox_row.child() {
                        if let Some(content) = row_box.downcast_ref::<gtk::Box>() {
                            if let Some(header_widget) = content.first_child() {
                                if let Some(header_row) = header_widget.downcast_ref::<gtk::Box>() {
                                    if let Some(label_widget) = header_row.first_child() {
                                        if let Some(label) =
                                            label_widget.downcast_ref::<gtk::Label>()
                                        {
                                            if label.text() == name {
                                                property_exists = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                row_child = row_widget.next_sibling();
            }

            if property_exists {
                GPS_INFO!("Property '{}' already exists, skipping duplicate", name);
                return;
            }

            // Add to update_properties
            update_properties
                .borrow_mut()
                .insert(name.clone(), value.clone());

            let name_for_change = name.clone();
            let name_for_remove = name.clone();

            // Add visual row to the listbox
            let row_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
            row_box.set_margin_start(12);
            row_box.set_margin_end(12);
            row_box.set_margin_top(8);
            row_box.set_margin_bottom(8);

            // Header row with property name and remove button
            let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);

            let property_name_label = gtk::Label::builder()
                .label(&name)
                .halign(gtk::Align::Start)
                .hexpand(true)
                .xalign(0.0)
                .build();
            header_row.append(&property_name_label);

            let remove_button = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text("Remove property")
                .halign(gtk::Align::End)
                .build();
            header_row.append(&remove_button);

            row_box.append(&header_row);

            let property_value_entry = gtk::Entry::new();
            property_value_entry.set_text(&value);
            property_value_entry.set_hexpand(true);

            let update_properties_clone = update_properties.clone();
            property_value_entry.connect_changed(glib::clone!(
                #[strong]
                update_properties_clone,
                move |entry| {
                    update_properties_clone
                        .borrow_mut()
                        .insert(name_for_change.clone(), entry.text().to_string());
                }
            ));

            row_box.append(&property_value_entry);

            let list_row = gtk::ListBoxRow::new();
            list_row.set_child(Some(&row_box));
            list_row.set_activatable(false);

            // Connect remove button to remove the row and property
            let update_properties_for_remove = update_properties.clone();
            let deleted_properties_for_remove = deleted_properties.clone();
            remove_button.connect_clicked(glib::clone!(
                #[weak]
                list_row,
                #[weak]
                parent_listbox,
                move |_| {
                    // Track this property for deletion when Apply is clicked
                    deleted_properties_for_remove
                        .borrow_mut()
                        .insert(name_for_remove.clone());
                    // Remove from pending updates (in case it was modified before deletion)
                    update_properties_for_remove
                        .borrow_mut()
                        .remove(&name_for_remove);
                    // Remove from UI immediately for visual feedback
                    parent_listbox.remove(&list_row);
                }
            ));

            parent_listbox.append(&list_row);

            // Clear the input fields
            property_name.set_text("");
            property_value.set_text("");
        }
    ));

    new_row_box.append(&add_button);

    let new_list_row = gtk::ListBoxRow::new();
    new_list_row.set_child(Some(&new_row_box));
    new_list_row.set_activatable(false);
    new_prop_listbox.append(&new_list_row);

    new_prop_box.append(&new_prop_listbox);
    content_box.append(&new_prop_box);

    scrolled.set_child(Some(&content_box));
    main_box.append(&scrolled);

    // Search functionality - filter existing pad properties
    let listbox_weak = listbox.downgrade();
    search_entry.connect_search_changed(move |entry| {
        let search_text = entry.text().to_string();
        if let Some(listbox) = listbox_weak.upgrade() {
            filter_pad_property_rows(&listbox, &search_text);
        }
    });

    // Convert Box to Grid for dialog compatibility
    let grid = gtk::Grid::new();
    grid.attach(&main_box, 0, 0, 1, 1);

    let dialog = GPSUI::dialog::create(
        &format!("{port_name} properties from {element_name}"),
        app,
        &grid,
        glib::clone!(
            #[strong]
            update_properties,
            #[strong]
            deleted_properties,
            move |app, _dialog| {
                // Merge updates and deletions into a single map for the backend.
                // Convention: Empty string indicates property deletion.
                // This allows a single API call to handle both updates and removals.
                let mut all_properties = update_properties.borrow().clone();

                // Mark deleted properties with empty string as deletion signal
                for deleted_name in deleted_properties.borrow().iter() {
                    all_properties.insert(deleted_name.clone(), String::new());
                }

                // Log the changes for debugging
                for (name, value) in all_properties.iter() {
                    GPS_INFO!(
                        "Property {}: {}",
                        name,
                        if value.is_empty() { "DELETED" } else { value }
                    );
                }

                // Send all changes (updates and deletions) to the backend
                app.update_pad_properties(node_id, port_id, &all_properties);
            }
        ),
    );

    dialog.set_default_size(600, 500);
    main_box.set_size_request(450, 400);
    dialog.present();
}

pub fn display_pipeline_details(app: &GPSApp) {
    let grid = gtk::Grid::new();
    grid.set_column_spacing(4);
    grid.set_row_spacing(4);
    grid.set_margin_bottom(12);

    if let Some(elements) = graphbook::current_graphtab(app)
        .player()
        .pipeline_elements()
    {
        let elements_list = elements.join(" ");
        let label = gtk::Label::builder()
            .label(format!("{} elements:", elements.len()))
            .hexpand(true)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Start)
            .margin_start(4)
            .build();

        let value = gtk::Label::builder()
            .label(elements_list)
            .hexpand(true)
            .halign(gtk::Align::Start)
            .margin_start(4)
            .wrap(true)
            .build();

        grid.attach(&label, 0, 0_i32, 1, 1);
        grid.attach(&value, 1, 0_i32, 1, 1);

        let dialog =
            GPSUI::dialog::create("Pipeline properties", app, &grid, move |_app, _dialog| {
                // Read-only dialog, Apply button does nothing
            });

        dialog.present();
    }
}
