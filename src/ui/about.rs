// about.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::GPSApp;
use crate::config;
use crate::gps as GPS;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::AboutDialog;

use gtk::ApplicationWindow;

pub fn display_about_dialog(app: &GPSApp) {
    let window: ApplicationWindow = app
        .builder
        .object("mainwindow")
        .expect("Couldn't get window");
    let about_dialog = AboutDialog::builder()
        .modal(true)
        .program_name("GstPipelineStudio")
        .version(config::VERSION)
        .comments(format!(
            "{}\n\n{}",
            &gettext("Draw your own GStreamer pipeline"),
            GPS::Player::get_version()
        ))
        .website("https://gitlab.freedesktop.org/dabrain34/GstPipelineStudio")
        .authors(vec!["Stéphane Cerveau".to_string()])
        .artists(vec!["Stéphane Cerveau".to_string()])
        .translator_credits(gettext("translator-credits"))
        .logo_icon_name(config::APP_ID)
        .license_type(gtk::License::Gpl30)
        .transient_for(&window)
        .build();

    about_dialog.show();
}
