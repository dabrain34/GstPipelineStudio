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
use gtk::glib;
use gtk::prelude::*;
use log::error;

use crate::app::GPSApp;
use crate::common::{init_gst, init_gtk};
use crate::ui::splash::create_splash_window;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Command {
    #[structopt(about = "Sets the pipeline description", default_value = "")]
    pipeline: String,
}

fn main() -> gtk::glib::ExitCode {
    //    gio::resources_register_include!("compiled.gresource").unwrap();

    // Initialize GTK first so we can show UI
    init_gtk().expect("Unable to init GTK");

    let application = gtk::Application::new(
        Some(config::APP_ID),
        gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE,
    );
    application.connect_startup(|application| {
        let args = Command::from_args();

        // Phase 1: Create and show the main window (empty)
        let gps_app = match GPSApp::create_window(application) {
            Some(app) => app,
            None => {
                error!("Failed to create application window");
                application.quit();
                return;
            }
        };

        // Show splash screen on top of the main window (centered on it)
        let splash_window = create_splash_window(&gps_app.window);

        // Create channel to signal when GStreamer init is complete
        let (init_tx, init_rx) = async_channel::bounded::<Result<(), String>>(1);

        // Spawn GStreamer initialization in a separate thread
        std::thread::spawn(move || {
            let result = init_gst().map_err(|e| e.to_string());
            let _ = init_tx.send_blocking(result);
        });

        // Wait for GStreamer init to complete, then proceed with app startup
        let app_clone = application.clone();
        let pipeline_desc = args.pipeline.clone();
        glib::spawn_future_local(async move {
            match init_rx.recv().await {
                Ok(Ok(())) => {
                    // GStreamer initialized successfully, close splash and initialize UI
                    splash_window.close();
                    // Phase 2: Initialize the UI content now that GStreamer is ready
                    gps_app.initialize_ui(&app_clone, &pipeline_desc);
                }
                Ok(Err(e)) => {
                    // GStreamer initialization failed
                    splash_window.close();
                    error!("Failed to initialize GStreamer: {}", e);
                    app_clone.quit();
                }
                Err(e) => {
                    // Channel error
                    splash_window.close();
                    error!("Internal error during initialization: {}", e);
                    app_clone.quit();
                }
            }
        });
    });

    application.connect_command_line(|_app, _cmd_line| {
        // structopt already handled arguments
        0.into()
    });
    application.run()
}
