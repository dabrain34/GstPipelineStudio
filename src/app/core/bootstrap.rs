// bootstrap.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

//! Application UI initialization and bootstrapping.
//!
//! Handles complete UI setup including window creation, logging system initialization,
//! action handlers, and loading initial graph state. Sets up the position slider
//! for playback control and manages video preview widgets.

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::{gio, Application, Label};

use crate::gps as GPS;
use crate::logger;
use crate::ui as GPSUI;
use crate::{GPS_DEBUG, GPS_ERROR, GPS_TRACE, GPS_WARN};

use super::super::settings::Settings;
use super::super::{AppState, GPSApp};
use super::graphbook;

const POSITION_UPDATE_TIMEOUT_MS: u64 = 100;

// Link colors for different pipeline states (RGB values 0.0-1.0)
const LINK_COLOR_PLAYING: (f64, f64, f64) = (0.2, 0.8, 0.2); // Green
const LINK_COLOR_PAUSED: (f64, f64, f64) = (1.0, 0.6, 0.0); // Orange
const LINK_COLOR_IDLE: (f64, f64, f64) = (0.5, 0.5, 0.5); // Gray

impl GPSApp {
    pub fn set_app_state(&self, state: AppState) {
        let status_bar: Label = self
            .builder
            .object("status_bar")
            .expect("Couldn't get status_bar");
        status_bar.set_text(&state.to_string());

        // Enable/disable pipeline details menu based on state
        // Only update if the action exists (may not exist during early initialization)
        if let Some(app) = gtk::gio::Application::default() {
            if let Some(action) = app
                .lookup_action("graph.pipeline_details")
                .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
            {
                let is_playing = matches!(state, AppState::Playing | AppState::Paused);
                action.set_enabled(is_playing);
            }
        }

        // Update link color on the active graph tab only
        let (r, g, b) = match state {
            AppState::Playing => LINK_COLOR_PLAYING,
            AppState::Paused => LINK_COLOR_PAUSED,
            _ => LINK_COLOR_IDLE,
        };
        graphbook::current_graphtab(self)
            .graphview()
            .set_link_color(r, g, b);

        // Show error dialog if in error state with a message
        if let AppState::Error(Some(ref msg)) = state {
            GPSUI::message::display_error_dialog(false, msg);
        }
    }

    pub fn set_app_preview(&self, paintable: &gdk::Paintable, n_sink: usize) {
        let picture = gtk::Picture::new();
        picture.set_paintable(Some(paintable));
        let notebook_preview: gtk::Notebook = self
            .builder
            .object("notebook-preview")
            .expect("Couldn't get box_preview");
        let mut n_video_sink = n_sink;
        for tab in self.graphbook.borrow().values() {
            n_video_sink += tab.player().n_video_sink();
        }

        if n_video_sink == 0 {
            loop {
                let i = notebook_preview.n_pages();
                if i == 0 {
                    break;
                }
                notebook_preview.remove_page(Some(i - 1));
            }
        }
        let box_preview = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        box_preview.append(&picture);
        let label = gtk::Label::new(Some(&format!("Preview{n_video_sink}")));
        notebook_preview.insert_page(&box_preview, Some(&label), None);
        notebook_preview.set_current_page(Some(n_video_sink as u32));
    }

    pub fn build_ui(&self, application: &Application, pipeline_desc: &String) {
        graphbook::setup_graphbook(self);
        graphbook::create_graphtab(self, 0, None);

        // Check for crash recovery BEFORE initializing logger (logger truncates the log file)
        let previous_log_content = if Settings::needs_crash_recovery() {
            let log_path = Settings::log_file_path();
            if log_path.exists() {
                std::fs::read_to_string(&log_path).ok()
            } else {
                None
            }
        } else {
            None
        };

        let (ready_tx, ready_rx) = async_channel::unbounded::<(logger::LogType, String)>();

        // Setup the logger to get messages into the TreeView
        // Note: This truncates the log file, so we read it above first

        logger::init_logger(
            ready_tx.clone(),
            Settings::log_file_path()
                .to_str()
                .expect("Unable to convert log file path to a string"),
        );
        logger::init_msg_logger(ready_tx);
        GPSUI::logger::setup_logger_list(self, "treeview-app-logger", logger::LogType::App);
        GPSUI::logger::setup_logger_list(self, "treeview-msg-logger", logger::LogType::Message);
        GPSUI::logger::setup_logger_list(self, "treeview-gst-logger", logger::LogType::Gst);
        let app_weak = self.downgrade();
        glib::spawn_future_local(async move {
            while let Ok(msg) = ready_rx.recv().await {
                let app = upgrade_weak!(app_weak, glib::ControlFlow::Break);
                GPSUI::logger::add_to_logger_list(&app, msg.0, &msg.1);
            }
            glib::ControlFlow::Continue
        });

        // Show crash recovery dialog if we have previous log content
        if let Some(log_content) = previous_log_content {
            if !log_content.is_empty() {
                GPSUI::message::display_crash_recovery_dialog(Some(&self.window), &log_content);
            }
        }
        // Mark the session as started (sets clean_shutdown = false)
        Settings::mark_session_start();

        let window = &self.window;

        window.present();
        self.set_app_state(AppState::Ready);
        self.setup_app_actions(application);
        self.update_recent_files_menu();

        let app_weak = self.downgrade();
        self.connect_app_menu_action("new-window", move |_, _| {
            let app = upgrade_weak!(app_weak);
            let id = graphbook::graphbook_get_new_graphtab_id(&app);
            graphbook::create_graphtab(&app, id, None);
            let graphbook: gtk::Notebook = app
                .builder
                .object("graphbook")
                .expect("Couldn't get graphbook");
            graphbook.set_current_page(Some(id));
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("open", move |_, _| {
            let app = upgrade_weak!(app_weak);
            GPSUI::dialog::get_file(
                &app,
                GPSUI::dialog::FileDialogType::Open,
                move |app, filename| {
                    app.load_graph(&filename, false)
                        .unwrap_or_else(|_| GPS_ERROR!("Unable to open file {}", filename));
                    Settings::add_recent_open_file(&filename);
                    app.update_recent_files_menu();
                },
            );
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("open_dot_folder", move |_, _| {
            let app = upgrade_weak!(app_weak);
            GPSUI::dialog::get_multiple_dot_files(&app, move |app, files| {
                for filename in files {
                    // Create new tab with filename stem as name
                    let tab_name = std::path::Path::new(&filename)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Untitled");

                    let id = graphbook::graphbook_get_new_graphtab_id(&app);
                    graphbook::create_graphtab(&app, id, Some(tab_name));

                    // Switch to the new tab
                    let graphbook: gtk::Notebook = app
                        .builder
                        .object("graphbook")
                        .expect("Couldn't get graphbook");
                    graphbook.set_current_page(Some(id));

                    // Load the dot file
                    app.load_graph(&filename, false)
                        .unwrap_or_else(|_| GPS_ERROR!("Unable to open dot file {}", filename));
                }
            });
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("open_pipeline", move |_, _| {
            let app = upgrade_weak!(app_weak);
            GPSUI::dialog::get_input(
                &app,
                "Enter pipeline description with gst-launch format",
                "description",
                &Settings::recent_pipeline_description(),
                move |app, pipeline_desc| {
                    app.load_pipeline(&pipeline_desc).unwrap_or_else(|_| {
                        GPS_ERROR!("Unable to open pipeline description {}", pipeline_desc)
                    });
                    Settings::set_recent_pipeline_description(&pipeline_desc);
                },
            );
        });

        // Listen: GPS as WebSocket server (for pipeline-snapshot tracer)
        let app_weak = self.downgrade();
        self.connect_app_menu_action("listen_pipeline", move |_, _| {
            let app = upgrade_weak!(app_weak);
            GPSUI::dialog::get_input(
                &app,
                "Listen for Pipeline",
                "WebSocket address",
                &Settings::websocket_description(),
                |app, ws_addr| {
                    Settings::set_websocket_description(&ws_addr);
                    app.start_websocket_server(&ws_addr);
                },
            );
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("save", move |_, _| {
            let app = upgrade_weak!(app_weak);
            let gt = graphbook::current_graphtab(&app);
            if gt.undefined() {
                GPSUI::dialog::get_file(
                    &app,
                    GPSUI::dialog::FileDialogType::Save,
                    move |app, filename| {
                        GPS_DEBUG!("Save file {}", filename);
                        app.save_graph(&filename)
                            .unwrap_or_else(|_| GPS_ERROR!("Unable to save file to {}", filename));
                        graphbook::current_graphtab_set_filename(&app, filename.as_str());
                    },
                );
            } else if gt.modified() {
                let filename = gt.filename();
                app.save_graph(&filename)
                    .unwrap_or_else(|_| GPS_ERROR!("Unable to save file to {}", filename));
                graphbook::current_graphtab_set_filename(&app, filename.as_str());
            }
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("save_as", move |_, _| {
            let app = upgrade_weak!(app_weak);
            GPSUI::dialog::get_file(
                &app,
                GPSUI::dialog::FileDialogType::Save,
                move |app, filename| {
                    GPS_DEBUG!("Save file {}", filename);
                    app.save_graph(&filename)
                        .unwrap_or_else(|_| GPS_ERROR!("Unable to save file to {}", filename));
                    graphbook::current_graphtab_set_filename(&app, filename.as_str());
                },
            );
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("preferences", move |_, _| {
            let app = upgrade_weak!(app_weak);
            GPSUI::preferences::display_settings(&app);
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("delete", move |_, _| {
            let app = upgrade_weak!(app_weak);
            graphbook::current_graphtab(&app)
                .graphview()
                .delete_selected();
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("undo", move |_, _| {
            let app = upgrade_weak!(app_weak);
            graphbook::current_graphtab(&app).graphview().undo();
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("redo", move |_, _| {
            let app = upgrade_weak!(app_weak);
            graphbook::current_graphtab(&app).graphview().redo();
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("about", move |_, _| {
            let app = upgrade_weak!(app_weak);
            GPSUI::about::display_about_dialog(&app);
        });

        let app_weak = self.downgrade();
        self.connect_button_action("button-play", move |_| {
            let app = upgrade_weak!(app_weak);
            let _ = graphbook::current_graphtab(&app).player().start_pipeline(
                &graphbook::current_graphtab(&app).graphview(),
                GPS::PipelineState::Playing,
            );
        });

        let app_weak = self.downgrade();
        self.connect_button_action("button-pause", move |_| {
            let app = upgrade_weak!(app_weak);
            let _ = graphbook::current_graphtab(&app).player().start_pipeline(
                &graphbook::current_graphtab(&app).graphview(),
                GPS::PipelineState::Paused,
            );
        });

        let app_weak = self.downgrade();
        self.connect_button_action("button-stop", move |_| {
            let app = upgrade_weak!(app_weak);

            let _ = graphbook::current_graphtab(&app)
                .player()
                .set_state(GPS::PipelineState::Stopped);
        });

        let app_weak = self.downgrade();
        self.connect_button_action("button-clear", move |_| {
            let app = upgrade_weak!(app_weak);
            app.clear_graph();
        });

        // Setup the favorite list
        GPSUI::elements::setup_favorite_list(self);
        // Setup the favorite list
        GPSUI::elements::setup_elements_list(self);
        if pipeline_desc.is_empty() {
            let _ = self
                .load_graph(
                    Settings::graph_file_path()
                        .to_str()
                        .expect("Unable to convert to string"),
                    true,
                )
                .map_err(|_e| {
                    GPS_WARN!("Unable to load default graph");
                });
        } else {
            self.load_pipeline(pipeline_desc).unwrap_or_else(|_| {
                GPS_ERROR!("Unable to open pipeline description {}", pipeline_desc)
            });
        }
    }

    pub fn setup_position_slider(&self) -> glib::SourceId {
        let app_weak = self.downgrade();
        let slider: gtk::Scale = self
            .builder
            .object("scale-position")
            .expect("Couldn't get scale-position");
        let slider_update_signal_id = slider.connect_value_changed(move |slider| {
            let app = upgrade_weak!(app_weak);
            let value = slider.value() as u64;
            GPS_TRACE!("Seeking to {} s", value);
            if graphbook::current_graphtab(&app)
                .player()
                .set_position(value)
                .is_err()
            {
                GPS_ERROR!("Seeking to {} failed", value);
            }
        });

        let app_weak = self.downgrade();
        glib::timeout_add_local(
            std::time::Duration::from_millis(POSITION_UPDATE_TIMEOUT_MS),
            move || {
                let app = upgrade_weak!(app_weak, glib::ControlFlow::Break);
                if graphbook::current_graphtab(&app).player().state() != GPS::PipelineState::Playing
                {
                    return glib::ControlFlow::Continue;
                }
                let label: gtk::Label = app
                    .builder
                    .object("label-position")
                    .expect("Couldn't get label-position");
                let slider: gtk::Scale = app
                    .builder
                    .object("scale-position")
                    .expect("Couldn't get scale-position");
                let position = graphbook::current_graphtab(&app).player().position();
                let duration = graphbook::current_graphtab(&app).player().duration();
                slider.set_range(0.0, duration as f64 / 1000_f64);
                slider.block_signal(&slider_update_signal_id);
                slider.set_value(position as f64 / 1000_f64);
                slider.unblock_signal(&slider_update_signal_id);

                // Query the current playing position from the underlying player.
                let position_desc = graphbook::current_graphtab(&app)
                    .player()
                    .position_description();
                // Display the playing position in the gui.
                label.set_text(&position_desc);
                // Tell the callback to continue calling this closure.
                glib::ControlFlow::Continue
            },
        )
    }
}
