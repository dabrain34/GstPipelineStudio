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
use gtk::{gio, gio::SimpleAction, glib, graphene};
use gtk::{
    Application, ApplicationWindow, Builder, Button, FileChooserAction, FileChooserDialog, Paned,
    PopoverMenu, ResponseType, Statusbar, Widget,
};
use log::error;
use once_cell::unsync::OnceCell;
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
#[derive(Debug)]
pub struct GPSAppInner {
    pub window: gtk::ApplicationWindow,
    pub current_graphtab: Cell<u32>,
    pub graphbook: RefCell<HashMap<u32, graphbook::GraphTab>>,
    pub builder: Builder,
    pub plugin_list_initialized: OnceCell<bool>,
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
            plugin_list_initialized: OnceCell::new(),
            signal_handlers: RefCell::new(HashMap::new()),
        }));
        let settings = Settings::load_settings();

        app.window
            .set_default_size(settings.app_width, settings.app_height);

        #[cfg(not(target_os = "macos"))]
        if settings.app_maximized {
            app.window.maximize();
        }

        app.set_paned_position(&settings, "graph_dashboard-paned", 100);
        app.set_paned_position(&settings, "graph_logs-paned", 100);
        app.set_paned_position(&settings, "elements_preview-paned", 100);
        app.set_paned_position(&settings, "elements_properties-paned", 100);
        app.set_paned_position(&settings, "playcontrols_position-paned", 100);

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
        let timeout_id =
            glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
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
            });

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
            app.save_paned_position(&mut settings, "graph_dashboard-paned");
            app.save_paned_position(&mut settings, "graph_logs-paned");
            app.save_paned_position(&mut settings, "elements_preview-paned");
            app.save_paned_position(&mut settings, "elements_properties-paned");
            app.save_paned_position(&mut settings, "playcontrols_position-paned");

            Settings::save_settings(&settings);

            let pop_menu: PopoverMenu = app
                .builder
                .object("app_pop_menu")
                .expect("Couldn't get app_pop_menu");
            pop_menu.unparent();
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

        application.add_action(&gio::SimpleAction::new("save_as", None));
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

        application.add_action(&gio::SimpleAction::new("logger.clear", None));

        application.add_action(&gio::SimpleAction::new("graph.check", None));
        application.add_action(&gio::SimpleAction::new("graph.clear", None));
        application.add_action(&gio::SimpleAction::new("graph.pipeline_details", None));

        application.add_action(&gio::SimpleAction::new("port.delete", None));
        application.add_action(&gio::SimpleAction::new("port.properties", None));

        application.add_action(&gio::SimpleAction::new("node.add-to-favorite", None));
        application.add_action(&gio::SimpleAction::new("node.delete", None));
        application.add_action(&gio::SimpleAction::new("node.request-pad-input", None));
        application.add_action(&gio::SimpleAction::new("node.request-pad-output", None));
        application.add_action(&gio::SimpleAction::new("node.properties", None));
    }

    pub fn app_pop_menu_at_position(
        &self,
        widget: &impl IsA<Widget>,
        x: f64,
        y: f64,
    ) -> PopoverMenu {
        let mainwindow: ApplicationWindow = self
            .builder
            .object("mainwindow")
            .expect("Couldn't get the main window");

        let pop_menu: PopoverMenu = self
            .builder
            .object("app_pop_menu")
            .expect("Couldn't get app_pop_menu");

        if let Some((x, y)) = widget.translate_coordinates(&mainwindow, x, y) {
            let point = graphene::Point::new(x as f32, y as f32);
            pop_menu.set_pointing_to(Some(&gdk::Rectangle::new(
                point.to_vec2().x() as i32,
                point.to_vec2().y() as i32,
                0,
                0,
            )));
        }
        pop_menu
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

    fn get_file_from_dialog<F: Fn(GPSApp, String) + 'static>(app: &GPSApp, save: bool, f: F) {
        let mut message = "Open file";
        let mut ok_button = "Open";
        let cancel_button = "Cancel";
        let mut action = FileChooserAction::Open;
        if save {
            message = "Save file";
            ok_button = "Save";
            action = FileChooserAction::Save;
        }
        let window: ApplicationWindow = app
            .builder
            .object("mainwindow")
            .expect("Couldn't get main window");
        let file_chooser: FileChooserDialog = FileChooserDialog::new(
            Some(message),
            Some(&window),
            action,
            &[
                (ok_button, ResponseType::Ok),
                (cancel_button, ResponseType::Cancel),
            ],
        );
        let app_weak = app.downgrade();
        file_chooser.connect_response(move |d: &FileChooserDialog, response: ResponseType| {
            let app = upgrade_weak!(app_weak);
            if response == ResponseType::Ok {
                let file = d.file().expect("Couldn't get file");
                let filename = String::from(
                    file.path()
                        .expect("Couldn't get file path")
                        .to_str()
                        .expect("Unable to convert to string"),
                );
                f(app, filename);
            }

            d.close();
        });

        file_chooser.show();
    }

    pub fn set_app_state(&self, state: AppState) {
        let status_bar: Statusbar = self
            .builder
            .object("status_bar")
            .expect("Couldn't get status_bar");
        status_bar.push(status_bar.context_id("Description"), &state.to_string());
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

        // Setup the logger to get messages into the TreeView
        let (ready_tx, ready_rx) = glib::MainContext::channel(glib::Priority::DEFAULT);
        let app_weak = self.downgrade();
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
        let _ = ready_rx.attach(None, move |msg: (logger::LogType, String)| {
            let app = upgrade_weak!(app_weak, glib::ControlFlow::Break);
            GPSUI::logger::add_to_logger_list(&app, msg.0, &msg.1);
            glib::ControlFlow::Continue
        });

        let window = &self.window;

        window.show();
        self.set_app_state(AppState::Ready);
        self.setup_app_actions(application);

        let pop_menu: PopoverMenu = self
            .builder
            .object("app_pop_menu")
            .expect("Couldn't get app_pop_menu");
        pop_menu.set_parent(window);

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
            GPSApp::get_file_from_dialog(&app, false, move |app, filename| {
                app.load_graph(&filename, false)
                    .unwrap_or_else(|_| GPS_ERROR!("Unable to open file {}", filename));
            });
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("open_pipeline", move |_, _| {
            let app = upgrade_weak!(app_weak);
            GPSUI::dialog::create_input_dialog(
                "Enter pipeline description with gst-launch format",
                "description",
                &Settings::recent_pipeline_description(),
                &app,
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
                GPSApp::get_file_from_dialog(&app, true, move |app, filename| {
                    GPS_DEBUG!("Save file {}", filename);
                    app.save_graph(&filename)
                        .unwrap_or_else(|_| GPS_ERROR!("Unable to save file to {}", filename));
                    graphbook::current_graphtab_set_filename(&app, filename.as_str());
                });
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
            GPSApp::get_file_from_dialog(&app, true, move |app, filename| {
                GPS_DEBUG!("Save file {}", filename);
                app.save_graph(&filename)
                    .unwrap_or_else(|_| GPS_ERROR!("Unable to save file to {}", filename));
                graphbook::current_graphtab_set_filename(&app, filename.as_str());
            });
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
        if GPS::ElementInfo::element_is_uri_src_handler(element_name) {
            GPSApp::get_file_from_dialog(self, false, move |app, filename| {
                GPS_DEBUG!("Open file {}", filename);
                let mut properties: HashMap<String, String> = HashMap::new();
                properties.insert(String::from("location"), filename);
                if let Some(node) = graphbook::current_graphtab(&app).graphview().node(node_id) {
                    node.update_properties(&properties);
                }
            });
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
    }

    pub fn update_pad_properties(
        &self,
        node_id: u32,
        port_id: u32,
        properties: &HashMap<String, String>,
    ) {
        let port = self.port(node_id, port_id);
        port.update_properties(properties);
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
        if !untitled {
            graphbook::current_graphtab_set_filename(self, filename);
        }
        Ok(())
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
