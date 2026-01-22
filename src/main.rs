// main.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

// Hide the console window on Windows release builds
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

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

use std::time::{Duration, Instant};

use crate::app::{GPSApp, SPLASH_MIN_DISPLAY_MS};
use crate::common::{init_gst, init_gtk};
use crate::ui::message::display_startup_error_dialog;
use crate::ui::splash::create_splash_window;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Command {
    #[structopt(about = "Sets the pipeline description", default_value = "")]
    pipeline: String,
}

/// Delay before showing splash to let main window stabilize
const SPLASH_SHOW_DELAY_MS: u64 = 100;

fn main() -> gtk::glib::ExitCode {
    // Initialize GTK first so we can show UI
    init_gtk().expect("Unable to init GTK");

    let application = gtk::Application::new(
        Some(config::APP_ID),
        gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE,
    );
    application.connect_startup(|application| {
        let args = Command::from_args();

        // Create and show main window first
        let gps_app = match GPSApp::create_window(application) {
            Some(app) => app,
            None => {
                GPS_ERROR!("Failed to create application window");
                application.quit();
                return;
            }
        };

        // Create channel to signal when GStreamer init is complete
        let (init_tx, init_rx) = async_channel::bounded::<Result<(), String>>(1);

        // Spawn GStreamer initialization in a separate thread
        std::thread::spawn(move || {
            let result = init_gst().map_err(|e| e.to_string());
            let _ = init_tx.send_blocking(result);
        });

        // Show splash after short delay to let main window stabilize
        let app_clone = application.clone();
        let pipeline_desc = args.pipeline.clone();

        glib::timeout_add_local_once(Duration::from_millis(SPLASH_SHOW_DELAY_MS), move || {
            // Present the main window first
            gps_app.present_window();

            // Defer splash creation to next event loop iteration so window manager
            // has time to position the main window (needed for proper centering)
            glib::idle_add_local_once(glib::clone!(
                #[strong]
                gps_app,
                move || {
                    let splash_window = create_splash_window(&gps_app.window);
                    let splash_start = Instant::now();

                    glib::spawn_future_local(async move {
                        // Wait for GStreamer initialization
                        let result = init_rx.recv().await;

                        // Ensure splash is displayed for at least SPLASH_MIN_DISPLAY_MS
                        let elapsed = splash_start.elapsed();
                        let min_display = Duration::from_millis(SPLASH_MIN_DISPLAY_MS);
                        if elapsed < min_display {
                            let remaining = min_display - elapsed;
                            glib::timeout_future(remaining).await;
                        }

                        // Close splash
                        splash_window.close();

                        // Initialize UI
                        match result {
                            Ok(Ok(())) => {
                                gps_app.initialize_ui(&app_clone, &pipeline_desc);
                            }
                            Ok(Err(e)) => {
                                let msg = format!("Failed to initialize GStreamer: {}", e);
                                GPS_ERROR!("{}", msg);
                                display_startup_error_dialog(
                                    Some(&gps_app.window),
                                    &app_clone,
                                    &msg,
                                );
                            }
                            Err(e) => {
                                let msg = format!("Internal error during initialization: {}", e);
                                GPS_ERROR!("{}", msg);
                                display_startup_error_dialog(
                                    Some(&gps_app.window),
                                    &app_clone,
                                    &msg,
                                );
                            }
                        }
                    });
                }
            ));
        });
    });

    application.connect_command_line(|_app, _cmd_line| {
        // structopt already handled arguments
        0.into()
    });
    application.run()
}
