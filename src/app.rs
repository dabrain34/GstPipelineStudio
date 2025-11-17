// app.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use glib::SignalHandlerId;
use gtk::gdk;
use gtk::prelude::*;
use gtk::{gio, gio::SimpleAction, glib};
use gtk::{Application, ApplicationWindow, Builder, Button, Label, Paned, PopoverMenu, Widget};
use log::error;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::ops;
use std::rc::{Rc, Weak};

use crate::gps as GPS;
use crate::graphbook;
use crate::logger;
use crate::settings::Settings;
use crate::ui as GPSUI;

use crate::{GPS_DEBUG, GPS_ERROR, GPS_TRACE, GPS_WARN};

use crate::graphmanager as GM;
use crate::graphmanager::PropertyExt;
use std::fmt;

// Constants for paned widget names
const PANED_GRAPH_DASHBOARD: &str = "graph_dashboard-paned";
const PANED_GRAPH_LOGS: &str = "graph_logs-paned";
const PANED_ELEMENTS_PREVIEW: &str = "elements_preview-paned";
const PANED_ELEMENTS_PROPERTIES: &str = "elements_properties-paned";

// Constants for default positions and ratios
const DEFAULT_PANED_POSITION: i32 = 100;
const PANED_RATIO_GRAPH: i32 = 3; // Graph area gets 4/5
const PANED_RATIO_TOTAL: i32 = 4;
const PANED_RATIO_ELEMENTS: i32 = 3; // Elements get 3/5 of their area
const MAXIMIZE_TIMEOUT_MS: u64 = 500;
const POSITION_UPDATE_TIMEOUT_MS: u64 = 500;
const MIN_PANED_SIZE: i32 = 100;
#[derive(Debug)]
pub struct GPSAppInner {
    pub window: gtk::ApplicationWindow,
    pub current_graphtab: Cell<u32>,
    pub graphbook: RefCell<HashMap<u32, graphbook::GraphTab>>,
    pub builder: Builder,
    pub signal_handlers: RefCell<HashMap<String, SignalHandlerId>>,
}

#[derive(Debug)]
pub enum AppState {
    Ready,
    Playing,
    Paused,
    Stopped,
    Error,
}

impl fmt::Display for AppState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}",)
    }
}

// This represents our main application window.
#[derive(Debug, Clone)]
pub struct GPSApp(Rc<GPSAppInner>);

// Deref into the contained struct to make usage a bit more ergonomic
impl ops::Deref for GPSApp {
    type Target = GPSAppInner;

    fn deref(&self) -> &GPSAppInner {
        &self.0
    }
}

// Weak reference to our application struct
//
// Weak references are important to prevent reference cycles. Reference cycles are cases where
// struct A references directly or indirectly struct B, and struct B references struct A again
// while both are using reference counting.
pub struct GPSAppWeak(Weak<GPSAppInner>);
impl GPSAppWeak {
    // Upgrade to a strong reference if it still exists
    pub fn upgrade(&self) -> Option<GPSApp> {
        self.0.upgrade().map(GPSApp)
    }
}

impl GPSApp {
    fn new(application: &gtk::Application) -> anyhow::Result<GPSApp> {
        let glade_src = include_str!("ui/gps.ui");
        let builder = Builder::from_string(glade_src);
        let window: ApplicationWindow = builder
            .object("mainwindow")
            .expect("Couldn't get the main window");
        window.set_application(Some(application));
        window.set_title(Some("GStreamer Pipeline Studio"));

        let app = GPSApp(Rc::new(GPSAppInner {
            window,
            current_graphtab: Cell::new(0),
            graphbook: RefCell::new(HashMap::new()),
            builder,
            signal_handlers: RefCell::new(HashMap::new()),
        }));
        let settings = Settings::load_settings();

        app.window
            .set_default_size(settings.app_width, settings.app_height);

        if settings.app_maximized {
            app.window.maximize();
        }

        app.set_paned_position(&settings, PANED_GRAPH_DASHBOARD, DEFAULT_PANED_POSITION);
        app.set_paned_position(&settings, PANED_GRAPH_LOGS, DEFAULT_PANED_POSITION);
        app.set_paned_position(&settings, PANED_ELEMENTS_PREVIEW, DEFAULT_PANED_POSITION);
        app.set_paned_position(&settings, PANED_ELEMENTS_PROPERTIES, DEFAULT_PANED_POSITION);

        Ok(app)
    }

    fn set_paned_position(
        &self,
        settings: &Settings,
        paned_name: &str,
        paned_default_position: i32,
    ) {
        let paned: Paned = self
            .builder
            .object(paned_name)
            .unwrap_or_else(|| panic!("Couldn't get {}", paned_name));
        paned.set_position(
            *settings
                .paned_positions
                .get(paned_name)
                .unwrap_or(&paned_default_position),
        );
    }

    fn save_paned_position(&self, settings: &mut Settings, paned_name: &str) {
        let paned: Paned = self
            .builder
            .object(paned_name)
            .unwrap_or_else(|| panic!("Couldn't get {}", paned_name));
        settings
            .paned_positions
            .insert(paned_name.to_string(), paned.position());
    }

    fn apply_paned_positions(&self, is_maximized: bool) {
        let graph_dashboard_paned: Paned = self
            .builder
            .object(PANED_GRAPH_DASHBOARD)
            .expect("Couldn't get graph_dashboard-paned");
        let graph_logs_paned: Paned = self
            .builder
            .object(PANED_GRAPH_LOGS)
            .expect("Couldn't get graph_logs-paned");
        let elements_preview_paned: Paned = self
            .builder
            .object(PANED_ELEMENTS_PREVIEW)
            .expect("Couldn't get elements_preview-paned");
        let elements_properties_paned: Paned = self
            .builder
            .object(PANED_ELEMENTS_PROPERTIES)
            .expect("Couldn't get elements_properties-paned");

        // Get the actual allocated dimensions
        let h_allocation = graph_dashboard_paned.allocation();
        let h_width = h_allocation.width();

        let v_allocation = graph_logs_paned.allocation();
        let v_height = v_allocation.height();

        if h_width > MIN_PANED_SIZE && v_height > MIN_PANED_SIZE {
            // Set horizontal split: graph area gets 4/5 of paned width
            let h_position = (h_width * PANED_RATIO_GRAPH) / PANED_RATIO_TOTAL;
            graph_dashboard_paned.set_position(h_position);

            // Set vertical split: graph gets 4/5 of paned height
            let v_position = (v_height * PANED_RATIO_GRAPH) / PANED_RATIO_TOTAL;
            graph_logs_paned.set_position(v_position);

            // Align elements_preview with graph_logs - use same position to align preview with logs
            elements_preview_paned.set_position(v_position);
            GPS_DEBUG!(
                "elements_preview_paned: aligned with logs at position={}",
                v_position
            );

            // Split elements from properties: 3/5 for elements, 2/5 for details
            let elements_properties_position =
                (v_position * PANED_RATIO_ELEMENTS) / PANED_RATIO_TOTAL;
            elements_properties_paned.set_position(elements_properties_position);
            GPS_DEBUG!(
                "elements_properties_paned: position={} (3/5 of v_position={})",
                elements_properties_position,
                v_position
            );

            let mode = if is_maximized {
                "Maximized"
            } else {
                "Windowed mode"
            };
            GPS_DEBUG!(
                "{} - Setting paned positions: h_width={}, v_height={}, h_pos={}, v_pos={}",
                mode,
                h_width,
                v_height,
                h_position,
                v_position
            );
        } else if !is_maximized {
            // Fallback to saved positions if allocation is not ready (only in windowed mode)
            let settings = Settings::load_settings();
            self.set_paned_position(&settings, PANED_GRAPH_DASHBOARD, 600);
            self.set_paned_position(&settings, PANED_GRAPH_LOGS, 400);
            GPS_DEBUG!("Windowed mode - Using saved positions");
        } else {
            GPS_WARN!(
                "Invalid paned sizes: h_width={}, v_height={}",
                h_width,
                v_height
            );
        }
    }

    pub fn on_startup(application: &gtk::Application, pipeline_desc: &String) {
        // Create application and error out if that fails for whatever reason
        let app = match GPSApp::new(application) {
            Ok(app) => app,
            Err(err) => {
                error!("Error creating application: {}", err);
                return;
            }
        };

        app.build_ui(application, pipeline_desc);

        // Setup dynamic paned positioning on maximize/unmaximize
        let app_clone_for_maximize = app.clone();
        let last_maximized_state = Rc::new(Cell::new(app.window.is_maximized()));

        app.window
            .connect_notify_local(Some("maximized"), move |window, _| {
                let is_maximized = window.is_maximized();

                // Only process if state actually changed
                if last_maximized_state.get() == is_maximized {
                    return;
                }
                last_maximized_state.set(is_maximized);

                let app = app_clone_for_maximize.clone();

                // Use timeout to ensure window is fully resized and allocated
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(MAXIMIZE_TIMEOUT_MS),
                    move || {
                        app.apply_paned_positions(is_maximized);
                    },
                );
            });

        // Setup dynamic paned positioning on window resize (for windowed mode)
        let app_clone_for_resize = app.clone();
        let resize_timeout_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

        app.window.connect_default_width_notify(glib::clone!(
            #[strong]
            resize_timeout_id,
            #[strong]
            app_clone_for_resize,
            move |window| {
                // Only apply resize in windowed mode (not when maximized)
                if window.is_maximized() {
                    return;
                }

                // Cancel any pending resize timeout
                if let Some(id) = resize_timeout_id.borrow_mut().take() {
                    id.remove();
                }

                let app = app_clone_for_resize.clone();
                let timeout_id_clone = resize_timeout_id.clone();

                // Use timeout to debounce resize events
                let new_id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(MAXIMIZE_TIMEOUT_MS),
                    move || {
                        app.apply_paned_positions(false);
                        timeout_id_clone.borrow_mut().take();
                    },
                );
                *resize_timeout_id.borrow_mut() = Some(new_id);
            }
        ));

        app.window.connect_default_height_notify(glib::clone!(
            #[strong]
            resize_timeout_id,
            #[strong]
            app_clone_for_resize,
            move |window| {
                // Only apply resize in windowed mode (not when maximized)
                if window.is_maximized() {
                    return;
                }

                // Cancel any pending resize timeout
                if let Some(id) = resize_timeout_id.borrow_mut().take() {
                    id.remove();
                }

                let app = app_clone_for_resize.clone();
                let timeout_id_clone = resize_timeout_id.clone();

                // Use timeout to debounce resize events
                let new_id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(MAXIMIZE_TIMEOUT_MS),
                    move || {
                        app.apply_paned_positions(false);
                        timeout_id_clone.borrow_mut().take();
                    },
                );
                *resize_timeout_id.borrow_mut() = Some(new_id);
            }
        ));

        let app_weak = app.downgrade();
        let slider: gtk::Scale = app
            .builder
            .object("scale-position")
            .expect("Couldn't get status_bar");
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
        let app_weak = app.downgrade();
        let timeout_id = glib::timeout_add_local(
            std::time::Duration::from_millis(POSITION_UPDATE_TIMEOUT_MS),
            move || {
                let app = upgrade_weak!(app_weak, glib::ControlFlow::Break);

                let label: gtk::Label = app
                    .builder
                    .object("label-position")
                    .expect("Couldn't get status_bar");
                let slider: gtk::Scale = app
                    .builder
                    .object("scale-position")
                    .expect("Couldn't get status_bar");
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
        );

        let timeout_id = RefCell::new(Some(timeout_id));
        let app_container = RefCell::new(Some(app));

        application.connect_shutdown(move |_| {
            let app = app_container
                .borrow_mut()
                .take()
                .expect("Shutdown called multiple times");
            let window: ApplicationWindow = app
                .builder
                .object("mainwindow")
                .expect("Couldn't get the main window");
            let mut settings = Settings::load_settings();
            settings.app_maximized = window.is_maximized();
            settings.app_width = window.default_width();
            settings.app_height = window.default_height();
            app.save_paned_position(&mut settings, PANED_GRAPH_DASHBOARD);
            app.save_paned_position(&mut settings, PANED_GRAPH_LOGS);
            app.save_paned_position(&mut settings, PANED_ELEMENTS_PREVIEW);
            app.save_paned_position(&mut settings, PANED_ELEMENTS_PROPERTIES);

            Settings::save_settings(&settings);
            if let Some(timeout_id) = timeout_id.borrow_mut().take() {
                timeout_id.remove();
            }

            app.drop();
        });
    }

    fn setup_app_actions(&self, application: &gtk::Application) {
        application.add_action(&gio::SimpleAction::new("new-window", None));
        application.set_accels_for_action("app.new-window", &["<primary>n"]);

        application.add_action(&gio::SimpleAction::new("open", None));
        application.set_accels_for_action("app.open", &["<primary>o"]);

        application.add_action(&gio::SimpleAction::new("open_pipeline", None));
        application.set_accels_for_action("app.open_pipeline", &["<primary>p"]);

        application.add_action(&gio::SimpleAction::new("save", None));
        application.set_accels_for_action("app.save", &["<primary>s"]);
        application.add_action(&gio::SimpleAction::new("save_as", None));

        application.add_action(&gio::SimpleAction::new("delete", None));
        application.set_accels_for_action("app.delete", &["<primary>d", "Delete"]);

        application.add_action(&gio::SimpleAction::new("preferences", None));
        application.set_accels_for_action("app.preferences", &["<primary>p"]);

        application.add_action(&gio::SimpleAction::new("about", None));
        application.set_accels_for_action("app.about", &["<primary>a"]);

        application.add_action(&gio::SimpleAction::new("favorite.remove", None));
        application.add_action(&gio::SimpleAction::new("element.add-to-favorite", None));

        application.add_action(&gio::SimpleAction::new("logger.clear", None));

        application.add_action(&gio::SimpleAction::new("graph.check", None));
        application.add_action(&gio::SimpleAction::new("graph.clear", None));

        let pipeline_details_action = gio::SimpleAction::new("graph.pipeline_details", None);
        pipeline_details_action.set_enabled(false); // Initially disabled
        application.add_action(&pipeline_details_action);

        application.add_action(&gio::SimpleAction::new("port.delete", None));
        application.add_action(&gio::SimpleAction::new("port.properties", None));

        application.add_action(&gio::SimpleAction::new("node.add-to-favorite", None));
        application.add_action(&gio::SimpleAction::new("node.delete", None));
        application.add_action(&gio::SimpleAction::new("node.request-pad-input", None));
        application.add_action(&gio::SimpleAction::new("node.request-pad-output", None));
        application.add_action(&gio::SimpleAction::new("node.properties", None));
        application.add_action(&gio::SimpleAction::new("node.duplicate", None));
    }

    pub fn app_pop_menu_at_position(
        &self,
        widget: &impl IsA<Widget>,
        x: f64,
        y: f64,
        menu_model: Option<&gio::MenuModel>,
    ) -> PopoverMenu {
        // Create a new PopoverMenu dynamically for GTK4
        let popover = PopoverMenu::builder().has_arrow(false).build();

        // Set the menu model if provided
        if let Some(model) = menu_model {
            popover.set_menu_model(Some(model));
        }

        // Set parent widget
        popover.set_parent(widget);

        // Set positioning
        let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
        popover.set_pointing_to(Some(&rect));

        // Use popup() which is the correct GTK4 method for context menus
        popover.popup();

        popover
    }

    pub fn show_context_menu_at_position(
        &self,
        widget: &impl IsA<Widget>,
        x: f64,
        y: f64,
        menu_model: &gio::MenuModel,
    ) {
        let popover = self.app_pop_menu_at_position(widget, x, y, Some(menu_model));

        // Set up auto-hide when menu item is activated
        popover.connect_closed(move |_| {
            // Context menu closed
        });
    }

    fn app_menu_action(&self, action_name: &str) -> SimpleAction {
        let application = gio::Application::default()
            .expect("No default application")
            .downcast::<gtk::Application>()
            .expect("Unable to downcast default application");

        application
            .lookup_action(action_name)
            .unwrap_or_else(|| panic!("Unable to find action {}", action_name))
            .dynamic_cast::<SimpleAction>()
            .expect("Unable to dynamic cast to SimpleAction")
    }

    pub fn disconnect_app_menu_action(&self, action_name: &str) {
        let action = self.app_menu_action(action_name);

        if let Some(signal_handler_id) = self.signal_handlers.borrow_mut().remove(action_name) {
            action.disconnect(signal_handler_id);
        }
    }

    pub fn connect_app_menu_action<
        F: Fn(&SimpleAction, std::option::Option<&glib::Variant>) + 'static,
    >(
        &self,
        action_name: &str,
        f: F,
    ) {
        let action = self.app_menu_action(action_name);
        self.disconnect_app_menu_action(action_name);
        let signal_handler_id = action.connect_activate(f);
        self.signal_handlers
            .borrow_mut()
            .insert(String::from(action_name), signal_handler_id);
    }

    fn connect_button_action<F: Fn(&Button) + 'static>(&self, button_name: &str, f: F) {
        let button: Button = self
            .builder
            .object(button_name)
            .unwrap_or_else(|| panic!("Couldn't get app_button {}", button_name));
        button.connect_clicked(f);
    }

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
        let (ready_tx, ready_rx) = async_channel::unbounded::<(logger::LogType, String)>();

        // Setup the logger to get messages into the TreeView

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

        let window = &self.window;

        window.present();
        self.set_app_state(AppState::Ready);
        self.setup_app_actions(application);

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
                },
            );
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

    // Downgrade to a weak reference
    pub fn downgrade(&self) -> GPSAppWeak {
        GPSAppWeak(Rc::downgrade(&self.0))
    }

    // Called when the application shuts down. We drop our app struct here
    fn drop(self) {}

    pub fn add_new_element(&self, element_name: &str) {
        let (inputs, outputs) = GPS::PadInfo::pads(element_name, false);
        let node = graphbook::current_graphtab(self)
            .graphview()
            .create_node(element_name, GPS::ElementInfo::element_type(element_name));
        let node_id = node.id();
        if let Some((prop_name, file_chooser)) =
            GPS::ElementInfo::element_is_uri_src_handler(element_name)
        {
            if file_chooser {
                GPSUI::dialog::get_file(
                    self,
                    GPSUI::dialog::FileDialogType::OpenAll,
                    move |app, filename| {
                        GPS_DEBUG!("Open file {}", filename);
                        let mut properties: HashMap<String, String> = HashMap::new();
                        properties.insert(prop_name.clone(), filename);
                        if let Some(node) =
                            graphbook::current_graphtab(&app).graphview().node(node_id)
                        {
                            node.update_properties(&properties);
                        }
                    },
                );
            } else {
                GPSUI::dialog::get_input(self, "Enter uri", "uri", "", move |app, uri| {
                    GPS_DEBUG!("Open uri {}", uri);
                    let mut properties: HashMap<String, String> = HashMap::new();
                    properties.insert(String::from("uri"), uri);
                    if let Some(node) = graphbook::current_graphtab(&app).graphview().node(node_id)
                    {
                        node.update_properties(&properties);
                    }
                });
            }
        } else if let Some((prop_name, file_chooser)) =
            GPS::ElementInfo::element_is_uri_sink_handler(element_name)
        {
            if file_chooser {
                GPSUI::dialog::get_file(
                    self,
                    GPSUI::dialog::FileDialogType::SaveAll,
                    move |app, filename| {
                        GPS_DEBUG!("Save file {}", filename);
                        let mut properties: HashMap<String, String> = HashMap::new();
                        properties.insert(prop_name.clone(), filename);
                        if let Some(node) =
                            graphbook::current_graphtab(&app).graphview().node(node_id)
                        {
                            node.update_properties(&properties);
                        }
                    },
                );
            } else {
                GPSUI::dialog::get_input(self, "Enter uri", "uri", "", move |app, uri| {
                    GPS_DEBUG!("Save uri {}", uri);
                    let mut properties: HashMap<String, String> = HashMap::new();
                    properties.insert(String::from("uri"), uri);
                    if let Some(node) = graphbook::current_graphtab(&app).graphview().node(node_id)
                    {
                        node.update_properties(&properties);
                    }
                });
            }
        }
        graphbook::current_graphtab(self).graphview().add_node(node);
        for input in inputs {
            self.create_port_with_caps(
                node_id,
                GM::PortDirection::Input,
                GM::PortPresence::Always,
                input.caps().to_string(),
            );
        }
        for output in outputs {
            self.create_port_with_caps(
                node_id,
                GM::PortDirection::Output,
                GM::PortPresence::Always,
                output.caps().to_string(),
            );
        }
    }

    pub fn node(&self, node_id: u32) -> GM::Node {
        let node = graphbook::current_graphtab(self)
            .graphview()
            .node(node_id)
            .unwrap_or_else(|| panic!("Unable to retrieve node with id {}", node_id));
        node
    }

    pub fn port(&self, node_id: u32, port_id: u32) -> GM::Port {
        let node = self.node(node_id);
        node.port(port_id)
            .unwrap_or_else(|| panic!("Unable to retrieve port with id {}", port_id))
    }

    pub fn update_element_properties(&self, node_id: u32, properties: &HashMap<String, String>) {
        let node = self.node(node_id);
        node.update_properties(properties);

        // Trigger graph update to save to cache file
        graphbook::current_graphtab(self)
            .graphview()
            .graph_updated();
    }

    pub fn update_pad_properties(
        &self,
        node_id: u32,
        port_id: u32,
        properties: &HashMap<String, String>,
    ) {
        let port = self.port(node_id, port_id);
        port.update_properties(properties);

        // Trigger graph update to save to cache file
        graphbook::current_graphtab(self)
            .graphview()
            .graph_updated();
    }

    pub fn element_property(&self, node_id: u32, property_name: &str) -> Option<String> {
        let node = self.node(node_id);
        PropertyExt::property(&node, property_name)
    }

    pub fn pad_properties(&self, node_id: u32, port_id: u32) -> HashMap<String, String> {
        let port = self.port(node_id, port_id);
        let mut properties: HashMap<String, String> = HashMap::new();
        for (name, value) in port.properties().iter() {
            if !port.hidden_property(name) {
                properties.insert(name.to_string(), value.to_string());
            }
        }
        properties
    }

    pub fn create_port_with_caps(
        &self,
        node_id: u32,
        direction: GM::PortDirection,
        presence: GM::PortPresence,
        caps: String,
    ) -> u32 {
        let node = self.node(node_id);
        let ports = node.all_ports(direction);
        let port_name = match direction {
            GM::PortDirection::Input => String::from("sink_"),
            GM::PortDirection::Output => String::from("src_"),
            _ => String::from("?"),
        };
        let port_name = format!("{}{}", port_name, ports.len());
        let port = graphbook::current_graphtab(self)
            .graphview()
            .create_port(&port_name, direction, presence);
        let id = port.id();
        let properties: HashMap<String, String> = HashMap::from([("_caps".to_string(), caps)]);
        port.update_properties(&properties);
        if let Some(mut node) = graphbook::current_graphtab(self).graphview().node(node_id) {
            graphbook::current_graphtab(self)
                .graphview()
                .add_port_to_node(&mut node, port);
        }
        id
    }

    pub fn create_link(
        &self,
        node_from_id: u32,
        node_to_id: u32,
        port_from_id: u32,
        port_to_id: u32,
    ) {
        let graphtab = graphbook::current_graphtab(self);
        let link =
            graphtab
                .graphview()
                .create_link(node_from_id, node_to_id, port_from_id, port_to_id);
        graphtab.graphview().add_link(link);
    }

    fn clear_graph(&self) {
        graphbook::current_graphtab(self).graphview().clear();
    }

    pub fn save_graph(&self, filename: &str) -> anyhow::Result<()> {
        let mut file = File::create(filename)?;
        let buffer = graphbook::current_graphtab(self).graphview().render_xml()?;
        file.write_all(&buffer)?;

        Ok(())
    }

    fn load_graph(&self, filename: &str, untitled: bool) -> anyhow::Result<()> {
        let mut file = File::open(filename)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).expect("buffer overflow");
        let graphtab = graphbook::current_graphtab(self);
        graphtab.graphview().load_from_xml(buffer)?;

        // Restore static pads for nodes that have no ports
        self.restore_static_pads();

        if !untitled {
            graphbook::current_graphtab_set_filename(self, filename);
        }
        Ok(())
    }

    fn restore_static_pads(&self) {
        let graphtab = graphbook::current_graphtab(self);
        let graphview = graphtab.graphview();
        let nodes = graphview.all_nodes(GM::NodeType::All);

        for node in nodes {
            // Check if node has no ports
            let has_ports = !node.ports().is_empty();

            if !has_ports {
                let node_id = node.id();
                let element_name = node.name();
                let position = node.position();

                GPS_DEBUG!(
                    "Restoring static pads for element: {} at position ({}, {})",
                    element_name,
                    position.0,
                    position.1
                );

                // Get static pads from GStreamer element factory
                let (inputs, outputs) = GPS::PadInfo::pads(&element_name, false);

                // Add input pads
                for input in inputs {
                    self.create_port_with_caps(
                        node_id,
                        GM::PortDirection::Input,
                        GM::PortPresence::Always,
                        input.caps().to_string(),
                    );
                }

                // Add output pads
                for output in outputs {
                    self.create_port_with_caps(
                        node_id,
                        GM::PortDirection::Output,
                        GM::PortPresence::Always,
                        output.caps().to_string(),
                    );
                }

                // Ensure position is preserved after adding ports
                if let Some(node) = graphview.node(node_id) {
                    GPS_DEBUG!(
                        "Position after adding ports: ({}, {})",
                        node.position().0,
                        node.position().1
                    );
                    // Re-apply position if it changed
                    if node.position() != position {
                        GPS_DEBUG!(
                            "Position changed! Restoring to ({}, {})",
                            position.0,
                            position.1
                        );
                        node.set_position(position.0, position.1);
                    }
                }
            }
        }
    }

    fn load_pipeline(&self, pipeline_desc: &str) -> anyhow::Result<()> {
        let graphtab = graphbook::current_graphtab(self);
        let pd_parsed = pipeline_desc.replace('\\', "");
        graphtab
            .player()
            .graphview_from_pipeline_description(&graphtab.graphview(), &pd_parsed);
        Ok(())
    }
}
