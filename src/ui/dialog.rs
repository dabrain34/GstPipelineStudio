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
use gtk::{ApplicationWindow, FileChooserAction, FileChooserDialog, FileFilter, ResponseType};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileDialogType {
    Save,
    Open,
    OpenAll,
    SaveAll,
}

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
    dialog.connect_response(glib::clone!(
        #[weak]
        dialog,
        move |_, _| {
            let app = upgrade_weak!(app_weak);
            f(app, dialog)
        }
    ));

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
    app: &GPSApp,
    dialog_name: &str,
    input_name: &str,
    default_value: &str,
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
    dialog.connect_response(glib::clone!(
        #[weak]
        entry,
        move |dialog, response_type| {
            let app = upgrade_weak!(app_weak);
            if response_type == gtk::ResponseType::Apply {
                f(app, entry.text().to_string());
            }
            dialog.close()
        }
    ));

    dialog.show();
}

pub fn get_file_from_dialog<F: Fn(GPSApp, String) + 'static>(
    app: &GPSApp,
    dlg_type: FileDialogType,
    f: F,
) {
    let mut message = "Open file";
    let mut ok_button = "Open";
    let cancel_button = "Cancel";
    let mut action = FileChooserAction::Open;
    if dlg_type == FileDialogType::Save || dlg_type == FileDialogType::SaveAll {
        message = "Save file";
        ok_button = "Save";
        action = FileChooserAction::Save;
    }
    let window: ApplicationWindow = app
        .builder
        .object("mainwindow")
        .expect("Couldn't get main window");
    let file_chooser: FileChooserDialog = FileChooserDialog::new(
        Some(message),
        Some(&window),
        action,
        &[
            (ok_button, ResponseType::Ok),
            (cancel_button, ResponseType::Cancel),
        ],
    );
    if dlg_type == FileDialogType::Save {
        file_chooser.set_current_name("untitled.gps");
    }
    if dlg_type == FileDialogType::Open {
        let filter = FileFilter::new();
        filter.add_pattern("*.gps");
        filter.set_name(Some("GPS Files (*.gps)"));
        file_chooser.add_filter(&filter);
    }

    let app_weak = app.downgrade();
    file_chooser.connect_response(move |d: &FileChooserDialog, response: ResponseType| {
        let app = upgrade_weak!(app_weak);
        if response == ResponseType::Ok {
            let file = d.file().expect("Couldn't get file");
            let filename = String::from(
                file.path()
                    .expect("Couldn't get file path")
                    .to_str()
                    .expect("Unable to convert to string"),
            );
            f(app, filename);
        }

        d.close();
    });

    file_chooser.show();
}
