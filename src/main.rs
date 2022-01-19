// main.rs
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
#[macro_use]
mod macros;
mod about;
mod app;
mod common;
mod config;
mod graphmanager;
#[macro_use]
mod logger;
mod gps;
mod plugindialogs;
mod settings;
use gtk::prelude::*;

use crate::app::GPSApp;
use crate::common::init;

fn main() {
    //    gio::resources_register_include!("compiled.gresource").unwrap();
    init().expect("Unable to init app");
    let application = gtk::Application::new(Some(config::APP_ID), Default::default());
    application.connect_startup(|application| {
        GPSApp::on_startup(application);
    });

    application.run();
}
