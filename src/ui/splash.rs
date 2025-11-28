// splash.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use gtk::{Box, Label, Orientation, Overlay, Picture, Spinner, Window};

use crate::config;
use crate::ui::resources::{load_app_css, SPLASH_BANNER_PNG};

const BANNER_WIDTH: i32 = 555;

/// Creates and shows a splash screen window during application initialization.
/// The splash is shown as a transient modal window on top of the parent window.
/// Returns the window handle so it can be closed when initialization completes.
pub fn create_splash_window(parent: &impl IsA<Window>) -> Window {
    // Ensure app CSS is loaded
    load_app_css();

    // Load and scale the banner image
    let (banner, splash_width, splash_height) =
        if let Ok(pixbuf) = Pixbuf::from_read(std::io::Cursor::new(SPLASH_BANNER_PNG)) {
            // Scale the image to fit the banner width while preserving aspect ratio
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
            // Fallback
            (Picture::new(), 500, 300)
        };

    // Use Window as transient for the parent - this centers it on the parent
    let window = Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Loading")
        .default_width(splash_width)
        .default_height(splash_height)
        .resizable(false)
        .decorated(false)
        .build();

    // Create overlay with banner as background
    let overlay = Overlay::new();
    overlay.set_child(Some(&banner));

    // Create overlay content (version, spinner, loading message)
    let overlay_box = Box::new(Orientation::Vertical, 8);
    overlay_box.set_halign(gtk::Align::Center);
    overlay_box.set_valign(gtk::Align::End);
    overlay_box.set_margin_bottom(20);

    // Version label
    let version_label = Label::new(Some(&format!("Version {}", config::VERSION)));
    version_label.add_css_class("splash-version");

    // Spinner to show activity
    let spinner = Spinner::new();
    spinner.set_spinning(true);
    spinner.set_size_request(24, 24);

    // Loading message
    let loading_label = Label::new(Some("Loading GStreamer registry..."));
    loading_label.add_css_class("splash-loading");

    overlay_box.append(&version_label);
    overlay_box.append(&spinner);
    overlay_box.append(&loading_label);

    overlay.add_overlay(&overlay_box);

    window.set_child(Some(&overlay));
    window.present();

    window
}
