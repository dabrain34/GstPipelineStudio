// preferences.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::GPSApp;

use crate::logger;
use crate::settings;
use crate::ui as GPSUI;
use gtk::glib;
use gtk::prelude::*;

fn add_settings_widget(grid: &gtk::Grid, label_name: &str, widget: &gtk::Widget, row: i32) {
    let label = gtk::Label::builder()
        .label(label_name)
        .hexpand(true)
        .halign(gtk::Align::Start)
        .margin_start(4)
        .build();

    grid.attach(&label, 0, row, 1, 1);
    grid.attach(widget, 1, row, 1, 1);
}

pub fn display_settings(app: &GPSApp) {
    let grid = gtk::Grid::new();
    grid.set_column_spacing(4);
    grid.set_row_spacing(4);
    grid.set_margin_bottom(12);
    let settings = settings::Settings::load_settings();
    let widget = gtk::CheckButton::new();
    widget.set_active(
        settings
            .preferences
            .get("use_gtk4_sink")
            .unwrap_or(&"true".to_string())
            .parse::<bool>()
            .expect("Should a boolean value"),
    );
    widget.connect_toggled(glib::clone!(move |c| {
        let mut settings = settings::Settings::load_settings();
        settings
            .preferences
            .insert("use_gtk4_sink".to_string(), c.is_active().to_string());
        settings::Settings::save_settings(&settings);
    }));

    let widget = widget
        .dynamic_cast::<gtk::Widget>()
        .expect("Should be a widget");
    add_settings_widget(
        &grid,
        "Use gtk4paintablesink element for video rendering:",
        &widget,
        0,
    );

    let widget = gtk::SpinButton::with_range(0.0, 5.0, 1.0);
    widget.set_value(
        settings
            .preferences
            .get("log_level")
            .unwrap_or(&"0.0".to_string())
            .parse::<f64>()
            .expect("Should a f64 value"),
    );
    widget.connect_value_changed(glib::clone!(move |c| {
        let mut settings = settings::Settings::load_settings();
        settings
            .preferences
            .insert("log_level".to_string(), c.value().to_string());
        logger::set_log_level(logger::LogLevel::from_u32(c.value() as u32));
        settings::Settings::save_settings(&settings);
    }));

    let widget = widget
        .dynamic_cast::<gtk::Widget>()
        .expect("Should be a widget");
    add_settings_widget(&grid, "Log level", &widget, 1);

    let dialog = GPSUI::dialog::create_dialog("Preferences", app, &grid, move |_app, dialog| {
        dialog.close();
    });

    let widget = gtk::Entry::new();
    widget.set_text(settings::Settings::gst_log_level().as_str());
    widget.connect_changed(glib::clone!(move |c| {
        let mut settings = settings::Settings::load_settings();
        settings
            .preferences
            .insert("gst_log_level".to_string(), c.text().to_string());
        settings::Settings::save_settings(&settings);
    }));
    let widget = widget
        .dynamic_cast::<gtk::Widget>()
        .expect("Should be a widget");
    add_settings_widget(&grid, "GST Log level", &widget, 2);
    dialog.show();
}
