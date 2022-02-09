// main.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

#[macro_use]
mod macros;
mod app;
mod common;
mod config;
mod graphmanager;
mod ui;
#[macro_use]
mod logger;
mod gps;
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
