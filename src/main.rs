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
mod graphbook;
mod graphmanager;
mod ui;
#[macro_use]
mod logger;
mod gps;
mod settings;
use gtk::prelude::*;

use crate::app::GPSApp;
use crate::common::init;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Command {
    #[structopt(about = "Sets the pipeline description", default_value = "")]
    pipeline: String,
}

fn main() {
    //    gio::resources_register_include!("compiled.gresource").unwrap();
    init().expect("Unable to init app");
    let application = gtk::Application::new(
        Some(config::APP_ID),
        gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE,
    );
    application.connect_startup(|application| {
        let args = Command::from_args();
        GPSApp::on_startup(application, &args.pipeline);
    });

    application.connect_command_line(|_app, _cmd_line| {
        // structopt already handled arguments
        0
    });
    application.run();
}
