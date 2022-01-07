// about.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
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
use crate::config;
use gettextrs::gettext;
use gtk::prelude::*;

use gtk::ApplicationWindow;

pub fn display_about_dialog(app: &GPSApp) {
    let window: ApplicationWindow = app
        .builder
        .object("mainwindow")
        .expect("Couldn't get window");
    let about_dialog = gtk::AboutDialogBuilder::new()
        .modal(true)
        .program_name("GstPipelineStudio")
        .version(config::VERSION)
        .comments(&gettext("Draw your own GStreamer pipeline"))
        .website("https://gitlab.freedesktop.org/dabrain34/GstPipelineStudio")
        .authors(vec!["Stéphane Cerveau".to_string()])
        .artists(vec!["Stéphane Cerveau".to_string()])
        .translator_credits(&gettext("translator-credits"))
        .logo_icon_name(config::APP_ID)
        .license_type(gtk::License::Gpl30)
        .transient_for(&window)
        .build();

    about_dialog.show();
}
