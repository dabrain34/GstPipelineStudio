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

    // Create a more GNOME-friendly dialog with proper title and details
    let (title, detail) = match message_type {
        gtk::MessageType::Error => ("Error", message),
        gtk::MessageType::Warning => ("Warning", message),
        gtk::MessageType::Info => ("Information", message),
        _ => ("Message", message),
    };

    let dialog = AlertDialog::builder()
        .message(title)
        .detail(detail)
        .modal(true)
        .buttons(["OK"])
        .default_button(0)
        .cancel_button(0)
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

/// Display an error dialog during startup when the application may not be fully initialized.
/// This variant takes explicit window and application references.
pub fn display_startup_error_dialog(
    window: Option<&impl IsA<gtk::Window>>,
    app: &gtk::Application,
    message: &str,
) {
    let dialog = AlertDialog::builder()
        .message("Initialization Error")
        .detail(message)
        .modal(true)
        .buttons(["OK"])
        .default_button(0)
        .cancel_button(0)
        .build();

    let app_weak = app.downgrade();
    dialog.choose(window, gio::Cancellable::NONE, move |_result| {
        if let Some(app) = app_weak.upgrade() {
            app.quit();
        }
    });
}
