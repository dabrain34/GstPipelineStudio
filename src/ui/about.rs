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
use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use gtk::AboutDialog;

use gtk::ApplicationWindow;

// Embed the logo PNG directly in the binary
static LOGO_PNG: &[u8] = include_bytes!("../../data/icons/org.freedesktop.dabrain34.GstPipelineStudio.png");

pub fn display_about_dialog(app: &GPSApp) {
    let window: ApplicationWindow = app
        .builder
        .object("mainwindow")
        .expect("Couldn't get window");

    // Load logo from embedded PNG
    let logo = Pixbuf::from_read(std::io::Cursor::new(LOGO_PNG))
        .ok()
        .map(|pixbuf| gtk::gdk::Texture::for_pixbuf(&pixbuf));

    let mut builder = AboutDialog::builder()
        .modal(true)
        .program_name("GstPipelineStudio")
        .version(config::VERSION)
        .comments(format!(
            "{}\n\nGTK: {}.{}.{}\nGStreamer: {}",
            &"Draw your own GStreamer pipeline",
            gtk::major_version(),
            gtk::minor_version(),
            gtk::micro_version(),
            GPS::Player::version()
        ))
        .website("https://gitlab.freedesktop.org/dabrain34/GstPipelineStudio")
        .authors(vec!["Stéphane Cerveau".to_string()])
        .artists(vec!["Stéphane Cerveau".to_string()])
        .translator_credits("translator-credits")
        .license_type(gtk::License::Gpl30)
        .transient_for(&window);

    if let Some(texture) = logo {
        builder = builder.logo(&texture);
    } else {
        builder = builder.logo_icon_name(config::APP_ID);
    }

    let about_dialog = builder.build();
    about_dialog.present();
}
