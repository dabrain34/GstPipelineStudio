// message.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use gtk::gio;
use gtk::prelude::*;

use gtk::{Application, Label, Widget};

pub fn display_message_dialog<F: Fn(Application) + 'static>(
    message: &str,
    message_type: gtk::MessageType,
    f: F,
) {
    let app = gio::Application::default()
        .expect("No default application")
        .downcast::<gtk::Application>()
        .expect("Default application has wrong type");

    let dialog = gtk::MessageDialog::new(
        app.active_window().as_ref(),
        gtk::DialogFlags::MODAL,
        message_type,
        gtk::ButtonsType::Ok,
        message,
    );
    let message_area = dialog.message_area();
    let mut child = message_area.first_child();
    while child.is_some() {
        let widget = child.unwrap();
        let label = widget
            .dynamic_cast::<Label>()
            .expect("unable to cast child to Label");
        label.set_selectable(true);
        let widget = label.dynamic_cast::<Widget>().unwrap();
        child = widget.next_sibling();
    }

    let app_weak = app.downgrade();
    dialog.connect_response(move |dialog, _| {
        let app = upgrade_weak!(app_weak);
        dialog.destroy();
        f(app);
    });
    dialog.set_resizable(false);
    dialog.show();
}

#[allow(dead_code)]
pub fn display_error_dialog(fatal: bool, message: &str) {
    display_message_dialog(
        &format!("Error: {}", message),
        gtk::MessageType::Error,
        move |app| {
            if fatal {
                app.quit();
            }
        },
    );
}
