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

/// Display a dialog showing the previous session's log after a crash.
/// Called when a crash is detected (previous session did not shut down cleanly).
/// Takes the log content directly since the log file gets truncated when the new logger initializes.
pub fn display_crash_recovery_dialog(
    parent_window: Option<&impl IsA<gtk::Window>>,
    log_content: &str,
) {
    // Create the dialog window - match parent width, half parent height
    let dialog = gtk::Window::builder()
        .title("Previous Session Crash Detected")
        .modal(true)
        .build();

    if let Some(parent) = parent_window {
        dialog.set_transient_for(Some(parent.as_ref()));

        // Try multiple methods to get parent size:
        // 1. Actual allocated size (works if window is realized)
        // 2. Default size (works if set programmatically)
        // 3. Fallback to reasonable defaults
        let parent_ref = parent.as_ref();
        let width = parent_ref.width();
        let height = parent_ref.height();

        if width > 100 && height > 100 {
            // Use actual size if window is already realized
            dialog.set_default_size(width, height / 2);
        } else {
            // Window not yet realized, try default_size
            let (def_width, def_height) = parent_ref.default_size();
            if def_width > 100 && def_height > 100 {
                dialog.set_default_size(def_width, def_height / 2);
            } else {
                // Fallback: use a large size
                dialog.set_default_size(1200, 350);
            }
        }
    } else {
        dialog.set_default_size(1200, 350);
    }

    // Create main content box
    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_start(10)
        .margin_end(10)
        .margin_top(10)
        .margin_bottom(10)
        .build();

    // Add explanation label
    let explanation = gtk::Label::builder()
        .label("The previous session did not shut down properly. This may indicate a crash.")
        .wrap(true)
        .xalign(0.0)
        .build();
    content_box.append(&explanation);

    // Create scrolled window with text view for log content (read-only, copy-pastable)
    let scrolled_window = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .has_frame(true)
        .build();

    let text_view = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(true)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .left_margin(6)
        .right_margin(6)
        .top_margin(6)
        .bottom_margin(6)
        .build();

    // Set log content
    let buffer = text_view.buffer();
    buffer.set_text(log_content);

    scrolled_window.set_child(Some(&text_view));
    content_box.append(&scrolled_window);

    dialog.set_child(Some(&content_box));
    dialog.present();
}
