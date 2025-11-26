// splash.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use gtk::prelude::*;
use gtk::{Box, CssProvider, Image, Label, Orientation, Spinner, Window};

use crate::config;

const SPLASH_WIDTH: i32 = 400;
const SPLASH_HEIGHT: i32 = 280;
const LOGO_SIZE: i32 = 64;

/// Creates and shows a splash screen window during application initialization.
/// The splash is shown as a transient modal window on top of the parent window.
/// Returns the window handle so it can be closed when initialization completes.
pub fn create_splash_window(parent: &impl IsA<Window>) -> Window {
    // Use Window as transient for the parent - this centers it on the parent
    let window = Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Loading")
        .default_width(SPLASH_WIDTH)
        .default_height(SPLASH_HEIGHT)
        .resizable(false)
        .decorated(false)
        .build();

    // Add CSS for splash screen styling with border
    let css_provider = CssProvider::new();
    css_provider.load_from_data(
        "
        .splash-container {
            background-color: @theme_bg_color;
            border: 2px solid @borders;
            border-radius: 8px;
        }
        ",
    );

    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("Could not get default display"),
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let container = Box::new(Orientation::Vertical, 20);
    container.add_css_class("splash-container");
    container.set_margin_top(30);
    container.set_margin_bottom(30);
    container.set_margin_start(30);
    container.set_margin_end(30);
    container.set_halign(gtk::Align::Center);
    container.set_valign(gtk::Align::Center);
    container.set_hexpand(true);
    container.set_vexpand(true);

    // App logo
    let logo = Image::from_icon_name(config::APP_ID);
    logo.set_pixel_size(LOGO_SIZE);

    // App title
    let title_label = Label::new(Some("GStreamer Pipeline Studio"));
    title_label.add_css_class("title-1");

    // Version label
    let version_label = Label::new(Some(&format!("Version {}", config::VERSION)));
    version_label.add_css_class("dim-label");

    // Loading message
    let loading_label = Label::new(Some("Loading GStreamer registry..."));
    loading_label.add_css_class("caption");

    // Spinner to show activity
    let spinner = Spinner::new();
    spinner.set_spinning(true);
    spinner.set_size_request(32, 32);

    container.append(&title_label);
    container.append(&logo);
    container.append(&version_label);
    container.append(&spinner);
    container.append(&loading_label);

    window.set_child(Some(&container));

    window.present();

    window
}
