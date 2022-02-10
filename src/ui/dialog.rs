// dialog.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::GPSApp;

use gtk::glib;
use gtk::prelude::*;

pub fn create_dialog<F: Fn(GPSApp, gtk::Dialog) + 'static>(
    name: &str,
    app: &GPSApp,
    grid: &gtk::Grid,
    f: F,
) -> gtk::Dialog {
    let dialog =
        gtk::Dialog::with_buttons(Some(name), Some(&app.window), gtk::DialogFlags::MODAL, &[]);

    dialog.set_default_size(640, 480);
    dialog.set_modal(true);
    let app_weak = app.downgrade();
    dialog.connect_response(glib::clone!(@weak dialog => move |_,_| {
        let app = upgrade_weak!(app_weak);
        f(app, dialog)
    }));

    let scrolledwindow = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    scrolledwindow.set_child(Some(grid));
    let content_area = dialog.content_area();
    content_area.append(&scrolledwindow);
    content_area.set_vexpand(true);
    content_area.set_margin_start(10);
    content_area.set_margin_end(10);
    content_area.set_margin_top(10);
    content_area.set_margin_bottom(10);

    dialog
}

pub fn create_input_dialog<F: Fn(GPSApp, String) + 'static>(
    dialog_name: &str,
    input_name: &str,
    default_value: &str,
    app: &GPSApp,
    f: F,
) {
    let dialog = gtk::Dialog::with_buttons(
        Some(dialog_name),
        Some(&app.window),
        gtk::DialogFlags::MODAL,
        &[("Ok", gtk::ResponseType::Apply)],
    );
    dialog.set_default_size(600, 100);
    dialog.set_modal(true);

    let label = gtk::Label::builder()
        .label(input_name)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Start)
        .margin_start(4)
        .build();

    let entry = gtk::Entry::builder()
        .width_request(400)
        .valign(gtk::Align::Center)
        .build();
    entry.set_text(default_value);

    let content_area = dialog.content_area();
    content_area.set_orientation(gtk::Orientation::Horizontal);
    content_area.set_vexpand(true);
    content_area.set_margin_start(10);
    content_area.set_margin_end(10);
    content_area.set_margin_top(10);
    content_area.set_margin_bottom(10);
    content_area.append(&label);
    content_area.append(&entry);
    let app_weak = app.downgrade();
    dialog.connect_response(glib::clone!(@weak entry => move |dialog, response_type| {
        let app = upgrade_weak!(app_weak);
        if response_type == gtk::ResponseType::Apply {
            f(app, entry.text().to_string());
        }
        dialog.close()
    }));

    dialog.show();
}
