// message.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use gtk::gio;
use gtk::prelude::*;

use gtk::{AlertDialog, Application};

pub fn display_message_dialog<F: Fn(Application) + 'static>(
    message: &str,
    message_type: gtk::MessageType,
    f: F,
) {
    let app = gio::Application::default()
        .expect("No default application")
        .downcast::<gtk::Application>()
        .expect("Default application has wrong type");

    // AlertDialog doesn't have built-in message types, so format the message
    let formatted_message = match message_type {
        gtk::MessageType::Error => format!("Error: {message}"),
        gtk::MessageType::Warning => format!("Warning: {message}"),
        gtk::MessageType::Info => format!("Information: {message}"),
        _ => message.to_string(),
    };

    let dialog = AlertDialog::builder()
        .message(&formatted_message)
        .modal(true)
        .buttons(["Ok"])
        .default_button(0)
        .build();

    let app_weak = app.downgrade();
    dialog.choose(
        app.active_window().as_ref(),
        gio::Cancellable::NONE,
        move |_result| {
            let app = upgrade_weak!(app_weak);
            f(app);
        },
    );
}

pub fn display_error_dialog(fatal: bool, message: &str) {
    display_message_dialog(message, gtk::MessageType::Error, move |app| {
        if fatal {
            app.quit();
        }
    });
}
