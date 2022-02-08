// dialog.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-only
use crate::app::GPSApp;

use gtk::glib;
use gtk::prelude::*;

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
    dialog.connect_response(glib::clone!(@weak dialog => move |_,_| {
        let app = upgrade_weak!(app_weak);
        f(app, dialog)
    }));

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
