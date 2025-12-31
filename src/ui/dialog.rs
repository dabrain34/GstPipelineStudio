// dialog.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::GPSApp;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::{ApplicationWindow, FileDialog, FileFilter};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileDialogType {
    Save,
    Open,
    OpenAll,
    SaveAll,
}

pub fn create<W, F>(name: &str, app: &GPSApp, content: &W, f: F) -> gtk::Window
where
    W: IsA<gtk::Widget>,
    F: Fn(GPSApp, gtk::Window) + 'static,
{
    let window = gtk::Window::builder()
        .title(name)
        .transient_for(&app.window)
        .modal(true)
        .default_width(640)
        .default_height(480)
        .build();

    let header_bar = gtk::HeaderBar::new();

    // Add Apply button to the header bar
    let apply_button = gtk::Button::with_label("Apply");
    apply_button.add_css_class("suggested-action");

    let app_weak = app.downgrade();
    apply_button.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| {
            let app = upgrade_weak!(app_weak);
            f(app.clone(), window.clone());
        }
    ));

    header_bar.pack_end(&apply_button);
    window.set_titlebar(Some(&header_bar));

    let scrolledwindow = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .margin_start(10)
        .margin_end(10)
        .margin_top(10)
        .margin_bottom(10)
        .build();
    scrolledwindow.set_child(Some(content.as_ref()));

    window.set_child(Some(&scrolledwindow));

    window
}

pub fn get_input<F: Fn(GPSApp, String) + 'static>(
    app: &GPSApp,
    dialog_name: &str,
    input_name: &str,
    default_value: &str,
    f: F,
) {
    let window = gtk::Window::builder()
        .title(dialog_name)
        .transient_for(&app.window)
        .modal(true)
        .default_width(600)
        .default_height(100)
        .build();

    let header_bar = gtk::HeaderBar::new();
    let ok_button = gtk::Button::with_label("Ok");

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

    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .vexpand(true)
        .margin_start(10)
        .margin_end(10)
        .margin_top(10)
        .margin_bottom(10)
        .build();
    content_box.append(&label);
    content_box.append(&entry);

    let app_weak = app.downgrade();
    let f = std::rc::Rc::new(f);
    let f_clone = f.clone();
    ok_button.connect_clicked(glib::clone!(
        #[weak]
        entry,
        #[weak]
        window,
        move |_| {
            let app = upgrade_weak!(app_weak);
            f_clone(app, entry.text().to_string());
            window.close();
        }
    ));

    // Allow Enter key in entry to trigger OK action
    let app_weak = app.downgrade();
    entry.connect_activate(glib::clone!(
        #[weak]
        window,
        move |entry| {
            let app = upgrade_weak!(app_weak);
            f(app, entry.text().to_string());
            window.close();
        }
    ));

    header_bar.pack_end(&ok_button);
    window.set_titlebar(Some(&header_bar));
    window.set_child(Some(&content_box));

    window.present();
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
        let filters = gio::ListStore::new::<FileFilter>();

        // Combined filter for all supported files (default)
        let all_filter = FileFilter::new();
        all_filter.add_pattern("*.gps");
        all_filter.add_pattern("*.dot");
        all_filter.set_name(Some("All Pipeline Files (*.gps, *.dot)"));
        filters.append(&all_filter);

        // GPS files filter
        let gps_filter = FileFilter::new();
        gps_filter.add_pattern("*.gps");
        gps_filter.set_name(Some("GPS Files (*.gps)"));
        filters.append(&gps_filter);

        // DOT files filter
        let dot_filter = FileFilter::new();
        dot_filter.add_pattern("*.dot");
        dot_filter.set_name(Some("DOT Files (*.dot)"));
        filters.append(&dot_filter);

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
