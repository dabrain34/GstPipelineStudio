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
use gtk::gio;
use gtk::{ApplicationWindow, FileDialog, FileFilter};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileDialogType {
    Save,
    Open,
    OpenAll,
    SaveAll,
}

pub fn create<F: Fn(GPSApp, gtk::Dialog) + 'static>(
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

pub fn get_input<F: Fn(GPSApp, String) + 'static>(
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

    dialog.present();
}

pub fn get_file<F: Fn(GPSApp, String) + 'static>(app: &GPSApp, dlg_type: FileDialogType, f: F) {
    let window: ApplicationWindow = app
        .builder
        .object("mainwindow")
        .expect("Couldn't get main window");

    let file_dialog = FileDialog::builder().modal(true).build();

    // Set title and accept button label based on dialog type
    if dlg_type == FileDialogType::Save || dlg_type == FileDialogType::SaveAll {
        file_dialog.set_title("Save file");
        file_dialog.set_accept_label(Some("Save"));
        file_dialog.set_initial_name(Some("untitled.gps"));
    } else {
        file_dialog.set_title("Open file");
        file_dialog.set_accept_label(Some("Open"));
    }

    // Set up file filter for Open dialogs
    if dlg_type == FileDialogType::Open {
        let filter = FileFilter::new();
        filter.add_pattern("*.gps");
        filter.set_name(Some("GPS Files (*.gps)"));

        let filters = gio::ListStore::new::<FileFilter>();
        filters.append(&filter);
        file_dialog.set_filters(Some(&filters));
    }

    let app_weak = app.downgrade();

    // Use the appropriate method based on dialog type
    if dlg_type == FileDialogType::Save || dlg_type == FileDialogType::SaveAll {
        file_dialog.save(Some(&window), None::<&gio::Cancellable>, move |result| {
            let app = upgrade_weak!(app_weak);
            if let Ok(file) = result {
                let filename = String::from(
                    file.path()
                        .expect("Couldn't get file path")
                        .to_str()
                        .expect("Unable to convert to string"),
                );
                f(app, filename);
            }
        });
    } else {
        file_dialog.open(Some(&window), None::<&gio::Cancellable>, move |result| {
            let app = upgrade_weak!(app_weak);
            if let Ok(file) = result {
                let filename = String::from(
                    file.path()
                        .expect("Couldn't get file path")
                        .to_str()
                        .expect("Unable to convert to string"),
                );
                f(app, filename);
            }
        });
    }
}
