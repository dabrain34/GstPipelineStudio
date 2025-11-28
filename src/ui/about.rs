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
use crate::ui::resources::{load_app_css, SPLASH_BANNER_PNG};
use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use gtk::{
    ApplicationWindow, Box, Button, Label, LinkButton, Orientation, Picture, ScrolledWindow, Window,
};

const BANNER_WIDTH: i32 = 500;

pub fn display_about_dialog(app: &GPSApp) {
    let parent: ApplicationWindow = app
        .builder
        .object("mainwindow")
        .expect("Couldn't get window");

    // Ensure app CSS is loaded
    load_app_css();

    // Load and scale the banner image with high-quality interpolation
    let (banner, dialog_width, _banner_height) =
        if let Ok(pixbuf) = Pixbuf::from_read(std::io::Cursor::new(SPLASH_BANNER_PNG)) {
            let orig_width = pixbuf.width();
            let orig_height = pixbuf.height();
            let scale = BANNER_WIDTH as f64 / orig_width as f64;
            let new_height = (orig_height as f64 * scale) as i32;

            // Use Hyper interpolation for highest quality downscaling
            let scaled_pixbuf = pixbuf
                .scale_simple(BANNER_WIDTH, new_height, gtk::gdk_pixbuf::InterpType::Hyper)
                .unwrap_or(pixbuf);

            let texture = gtk::gdk::Texture::for_pixbuf(&scaled_pixbuf);
            let picture = Picture::for_paintable(&texture);
            picture.set_size_request(BANNER_WIDTH, new_height);
            (picture, BANNER_WIDTH, new_height)
        } else {
            (Picture::new(), 500, 150)
        };

    // Create the dialog window
    let window = Window::builder()
        .transient_for(&parent)
        .modal(true)
        .title("About GstPipelineStudio")
        .default_width(dialog_width)
        .resizable(false)
        .build();

    // Main vertical layout
    let main_box = Box::new(Orientation::Vertical, 0);

    // Banner at top
    main_box.append(&banner);

    // Content area with padding
    let content_box = Box::new(Orientation::Vertical, 8);
    content_box.add_css_class("about-content-box");
    content_box.set_margin_top(16);
    content_box.set_margin_bottom(16);
    content_box.set_margin_start(20);
    content_box.set_margin_end(20);

    // App name
    let name_label = Label::new(Some("GstPipelineStudio"));
    name_label.add_css_class("about-title");

    // Version
    let version_label = Label::new(Some(config::VERSION));
    version_label.add_css_class("about-version");

    // Description with versions
    let description = format!(
        "Draw your own GStreamer pipeline\n\nGTK: {}.{}.{}\nGStreamer: {}",
        gtk::major_version(),
        gtk::minor_version(),
        gtk::micro_version(),
        GPS::Player::version()
    );
    let description_label = Label::new(Some(&description));
    description_label.add_css_class("about-description");
    description_label.set_justify(gtk::Justification::Center);

    // Website link
    let website_button =
        LinkButton::new("https://gitlab.freedesktop.org/dabrain34/GstPipelineStudio");
    website_button.set_label("Project Website");
    website_button.add_css_class("about-link");

    // Button row
    let button_box = Box::new(Orientation::Horizontal, 8);
    button_box.set_halign(gtk::Align::Center);
    button_box.set_margin_top(8);

    // Credits button
    let credits_button = Button::with_label("Credits");
    let parent_weak = window.downgrade();
    credits_button.connect_clicked(move |_| {
        if let Some(parent) = parent_weak.upgrade() {
            show_credits_dialog(&parent);
        }
    });

    // License button
    let license_button = Button::with_label("License");
    let parent_weak = window.downgrade();
    license_button.connect_clicked(move |_| {
        if let Some(parent) = parent_weak.upgrade() {
            show_license_dialog(&parent);
        }
    });

    button_box.append(&credits_button);
    button_box.append(&license_button);

    // Copyright
    let copyright_label = Label::new(Some("© 2021-2025 Stéphane Cerveau"));
    copyright_label.add_css_class("about-copyright");
    copyright_label.set_margin_top(8);

    content_box.append(&name_label);
    content_box.append(&version_label);
    content_box.append(&description_label);
    content_box.append(&website_button);
    content_box.append(&button_box);
    content_box.append(&copyright_label);

    main_box.append(&content_box);
    window.set_child(Some(&main_box));

    // Close on Escape key press
    let key_controller = gtk::EventControllerKey::new();
    let window_weak = window.downgrade();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(w) = window_weak.upgrade() {
                w.close();
            }
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    window.add_controller(key_controller);

    window.present();
}

fn show_credits_dialog(parent: &Window) {
    let dialog = Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Credits")
        .default_width(300)
        .default_height(200)
        .resizable(false)
        .build();

    let content = Box::new(Orientation::Vertical, 12);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);

    let authors_title = Label::new(Some("Written by"));
    authors_title.add_css_class("about-version");

    let authors = Label::new(Some("Stéphane Cerveau"));
    authors.add_css_class("about-description");

    let artists_title = Label::new(Some("Artwork by"));
    artists_title.add_css_class("about-version");
    artists_title.set_margin_top(12);

    let artists = Label::new(Some("Stéphane Cerveau"));
    artists.add_css_class("about-description");

    content.append(&authors_title);
    content.append(&authors);
    content.append(&artists_title);
    content.append(&artists);

    dialog.set_child(Some(&content));

    // Close on Escape
    let key_controller = gtk::EventControllerKey::new();
    let dialog_weak = dialog.downgrade();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(d) = dialog_weak.upgrade() {
                d.close();
            }
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    dialog.add_controller(key_controller);

    dialog.present();
}

fn show_license_dialog(parent: &Window) {
    let dialog = Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("License")
        .default_width(500)
        .default_height(400)
        .build();

    let content = Box::new(Orientation::Vertical, 8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let title = Label::new(Some("GNU General Public License v3.0"));
    title.add_css_class("about-version");

    let scrolled = ScrolledWindow::new();
    scrolled.set_vexpand(true);

    let license_label = Label::new(Some(
        "This program is free software: you can redistribute it and/or modify \
        it under the terms of the GNU General Public License as published by \
        the Free Software Foundation, either version 3 of the License, or \
        (at your option) any later version.\n\n\
        This program is distributed in the hope that it will be useful, \
        but WITHOUT ANY WARRANTY; without even the implied warranty of \
        MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the \
        GNU General Public License for more details.\n\n\
        You should have received a copy of the GNU General Public License \
        along with this program. If not, see <https://www.gnu.org/licenses/>.",
    ));
    license_label.set_wrap(true);
    license_label.set_xalign(0.0);
    license_label.add_css_class("about-description");

    scrolled.set_child(Some(&license_label));

    content.append(&title);
    content.append(&scrolled);

    dialog.set_child(Some(&content));

    // Close on Escape
    let key_controller = gtk::EventControllerKey::new();
    let dialog_weak = dialog.downgrade();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(d) = dialog_weak.upgrade() {
                d.close();
            }
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    dialog.add_controller(key_controller);

    dialog.present();
}
