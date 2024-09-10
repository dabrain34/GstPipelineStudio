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
            let combo = gtk::ComboBoxText::new();

            combo.set_widget_name(property_name);
            GPS_TRACE!("add ComboBox property : {}", combo.widget_name());
            // Add an empty entry to be able to reset the value
            combo.append_text("");
            if t.is_a(glib::ParamSpecEnum::static_type()) {
                let param = param
                    .clone()
                    .downcast::<glib::ParamSpecEnum>()
                    .expect("Should be a ParamSpecEnum");
                let enums = param.enum_class();
                for value in enums.values() {
                    combo.append_text(&format!(
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
                    combo.append_text(&format!(
                        "{}:{}:{}",
                        value.value(),
                        value.nick(),
                        value.name()
                    ));
                }
            }
            if let Some(value) = app.element_property(node_id, property_name) {
                //Retrieve the first value (index) from the property
                combo.set_active(Some(value.parse::<u32>().unwrap_or(0) + 1));
            } else if (param.flags() & glib::ParamFlags::READABLE) == glib::ParamFlags::READABLE
                || (param.flags() & glib::ParamFlags::READWRITE) == glib::ParamFlags::READWRITE
            {
                if let Ok(value) =
                    GPS::ElementInfo::element_property_by_feature_name(element_name, param.name())
                {
                    combo.set_active(Some(value.parse::<u32>().unwrap_or(0) + 1));
                }
            }

            combo.connect_changed(move |c| {
                if let Some(text) = c.active_text() {
                    let value = text.to_string();
                    let value = value.split_once(':');
                    f(
                        c.widget_name().to_string(),
                        value.unwrap_or_default().0.to_string(),
                    );
                }
            });
            Some(combo.upcast::<gtk::Widget>())
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

pub fn display_plugin_properties(app: &GPSApp, element_name: &str, node_id: u32) {
    let update_properties: Rc<RefCell<HashMap<String, String>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let properties = GPS::ElementInfo::element_properties_by_feature_name(element_name).unwrap();

    let grid = gtk::Grid::new();
    grid.set_column_spacing(4);
    grid.set_row_spacing(4);
    grid.set_margin_bottom(12);

    let mut properties: Vec<(&String, &glib::ParamSpec)> = properties.iter().collect();
    properties.sort_by(|a, b| a.0.cmp(b.0));
    let mut i = 0;
    for (name, param) in properties {
        //Entry
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
            let label = gtk::Label::builder()
                .label(name)
                .hexpand(true)
                .halign(gtk::Align::Start)
                .margin_start(4)
                .build();
            grid.attach(&label, 0, i, 1, 1);
            grid.attach(&widget, 1, i, 1, 1);
            i += 1;
        }
    }

    let dialog = GPSUI::dialog::create_dialog(
        &format!("{element_name} properties"),
        app,
        &grid,
        glib::clone!(
            #[strong]
            update_properties,
            move |app, dialog| {
                app.update_element_properties(node_id, &update_properties.borrow());
                dialog.close();
            }
        ),
    );

    dialog.show();
}

pub fn display_pad_properties(
    app: &GPSApp,
    element_name: &str,
    port_name: &str,
    node_id: u32,
    port_id: u32,
) {
    let update_properties: Rc<RefCell<HashMap<String, String>>> =
        Rc::new(RefCell::new(HashMap::new()));

    let grid = gtk::Grid::new();
    grid.set_column_spacing(4);
    grid.set_row_spacing(4);
    grid.set_margin_bottom(12);

    let mut i = 0;
    let properties = app.pad_properties(node_id, port_id);
    for (name, value) in properties {
        let property_name = gtk::Label::builder()
            .label(&name)
            .hexpand(true)
            .halign(gtk::Align::Start)
            .margin_start(4)
            .build();
        let property_value = gtk::Entry::new();
        property_value.set_width_request(150);
        property_value.set_text(&value);
        property_value.connect_changed(glib::clone!(
            #[weak]
            property_name,
            #[weak]
            property_value,
            #[strong]
            update_properties,
            move |_| {
                update_properties.borrow_mut().insert(
                    property_name.text().to_string(),
                    property_value.text().to_string(),
                );
            }
        ));
        grid.attach(&property_name, 0, i, 1, 1);
        grid.attach(&property_value, 1, i, 1, 1);
        i += 1;
    }

    // Add a new property  allowing to set pads property.
    let label = gtk::Label::builder()
        .label("Add a new Property")
        .hexpand(true)
        .halign(gtk::Align::Start)
        .margin_start(4)
        .build();

    let property_name = gtk::Entry::new();
    property_name.set_width_request(150);
    let property_value = gtk::Entry::new();
    property_value.set_width_request(150);

    property_name.connect_changed(glib::clone!(
        #[weak]
        property_name,
        #[weak]
        property_value,
        #[strong]
        update_properties,
        move |_| {
            update_properties.borrow_mut().insert(
                property_name.text().to_string(),
                property_value.text().to_string(),
            );
        }
    ));

    property_value.connect_changed(glib::clone!(
        #[weak]
        property_name,
        #[weak]
        property_value,
        #[strong]
        update_properties,
        move |_| {
            update_properties.borrow_mut().insert(
                property_name.text().to_string(),
                property_value.text().to_string(),
            );
        }
    ));
    grid.attach(&label, 0, i, 1, 1);
    grid.attach(&property_name, 1, i, 1, 1);
    grid.attach(&property_value, 2, i, 1, 1);

    // Add all specific properties from the given element

    let dialog = GPSUI::dialog::create_dialog(
        &format!("{port_name} properties from {element_name}"),
        app,
        &grid,
        glib::clone!(
            #[strong]
            update_properties,
            move |app, dialog| {
                for p in update_properties.borrow().values() {
                    GPS_INFO!("updated properties {}", p);
                }
                app.update_pad_properties(node_id, port_id, &update_properties.borrow());
                dialog.close();
            }
        ),
    );

    dialog.show();
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
            GPSUI::dialog::create_dialog("Pipeline properties", app, &grid, move |_app, dialog| {
                dialog.close();
            });

        dialog.show();
    }
}
