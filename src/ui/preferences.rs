// preferences.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::GPSApp;

use crate::settings;
use crate::ui as GPSUI;
use gtk::glib;
use gtk::prelude::*;

pub fn display_settings(app: &GPSApp) {
    let grid = gtk::Grid::new();
    grid.set_column_spacing(4);
    grid.set_row_spacing(4);
    grid.set_margin_bottom(12);

    let label = gtk::Label::builder()
        .label("Use gtk4paintablesink element for video rendering:")
        .hexpand(true)
        .halign(gtk::Align::Start)
        .margin_start(4)
        .build();
    let widget = gtk::CheckButton::new();
    let settings = settings::Settings::load_settings();
    widget.set_active(
        settings
            .preferences
            .get("use_gtk4_sink")
            .unwrap_or(&"true".to_string())
            .parse::<bool>()
            .expect("Should a boolean value"),
    );
    widget.connect_toggled(glib::clone!(@weak widget => move |c| {
        let mut settings = settings::Settings::load_settings();
        settings.preferences.insert("use_gtk4_sink".to_string(), c.is_active().to_string());
        settings::Settings::save_settings(&settings);
    }));

    grid.attach(&label, 0, 0, 1, 1);
    grid.attach(&widget, 1, 0, 1, 1);

    let dialog = GPSUI::dialog::create_dialog("Preferences", app, &grid, move |_app, dialog| {
        dialog.close();
    });

    dialog.show();
}
