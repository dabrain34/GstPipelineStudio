// app.rs
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
use gtk::prelude::*;
use gtk::{gio, glib};
use gtk::{
    AboutDialog, Application, ApplicationWindow, Builder, Button, FileChooserAction,
    FileChooserDialog, ResponseType, Statusbar, Viewport,
};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::{error, ops};

use crate::pipeline::Pipeline;
use crate::pluginlist;

use crate::graphmanager::{GraphView, Node};

static STYLE: &str = include_str!("style.css");
#[derive(Debug)]
pub struct GPSAppInner {
    pub window: gtk::ApplicationWindow,
    pub graphview: RefCell<GraphView>,
    pub builder: Builder,
    pub pipeline: RefCell<Pipeline>,
}

// This represents our main application window.
#[derive(Debug, Clone)]
pub struct GPSApp(Rc<GPSAppInner>);

// Deref into the contained struct to make usage a bit more ergonomic
impl ops::Deref for GPSApp {
    type Target = GPSAppInner;

    fn deref(&self) -> &GPSAppInner {
        &*self.0
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
    fn new(application: &gtk::Application) -> anyhow::Result<GPSApp, Box<dyn error::Error>> {
        let glade_src = include_str!("gps.ui");
        let builder = Builder::from_string(glade_src);
        let window: ApplicationWindow = builder.object("mainwindow").expect("Couldn't get window");
        window.set_application(Some(application));
        window.set_title(Some("GstPipelineStudio"));
        window.set_size_request(800, 600);
        let pipeline = Pipeline::new().expect("Unable to initialize the pipeline");
        let app = GPSApp(Rc::new(GPSAppInner {
            window,
            graphview: RefCell::new(GraphView::new()),
            builder,
            pipeline: RefCell::new(pipeline),
        }));
        Ok(app)
    }
    pub fn on_startup(application: &gtk::Application) {
        // Create application and error out if that fails for whatever reason
        let app = match GPSApp::new(application) {
            Ok(app) => app,
            Err(_err) => {
                /*                 utils::show_error_dialog(
                    true,
                    format!("Error creating application: {}", err).as_str(),
                ); */
                return;
            }
        };

        // When the application is activated show the UI. This happens when the first process is
        // started, and in the first process whenever a second process is started
        let app_weak = app.downgrade();

        application.connect_activate(glib::clone!(@weak application => move |_| {
            let app = upgrade_weak!(app_weak);
            app.build_ui(&application);
        }));

        let app_container = RefCell::new(Some(app));
        application.connect_shutdown(move |_| {
            let app = app_container
                .borrow_mut()
                .take()
                .expect("Shutdown called multiple times");
            app.drop();
        });

        // Load CSS from the STYLE variable.
        let provider = gtk::CssProvider::new();
        provider.load_from_data(STYLE.as_bytes());
        gtk::StyleContext::add_provider_for_display(
            &gtk::gdk::Display::default().expect("Error initializing gtk css provider."),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    pub fn build_ui(&self, application: &Application) {
        let drawing_area_window: Viewport = self
            .builder
            .object("drawing_area")
            .expect("Couldn't get window");

        drawing_area_window.set_child(Some(&*self.graphview.borrow()));

        let window = &self.window;

        window.show();
        let status_bar: Statusbar = self
            .builder
            .object("status_bar")
            .expect("Couldn't get window");
        status_bar.push(status_bar.context_id("Description"), "GPS is ready");

        let action = gio::SimpleAction::new("open", None);
        let app_weak = self.downgrade();
        // Add a dialog to open the graph
        action.connect_activate(glib::clone!(@weak window => move |_, _| {
                let app = upgrade_weak!(app_weak);
                let file_chooser = FileChooserDialog::new(
                    Some("Open File"),
                    Some(&window),
                    FileChooserAction::Open,
                    &[("Open", ResponseType::Ok), ("Cancel", ResponseType::Cancel)],
                );
                file_chooser.connect_response(move |d: &FileChooserDialog, response: ResponseType| {
                    if response == ResponseType::Ok {
                        let file = d.file().expect("Couldn't get file");
                        let filename = String::from(file.path().expect("Couldn't get file path").to_str().expect("unable to convert to string"));
                        println!("Open file {}", filename);
                        app.load_graph(&filename).expect("Unable to open file");
                    }

                    d.close();
                });

                file_chooser.show();

        }));
        application.add_action(&action);
        application.set_accels_for_action("app.open", &["<primary>o"]);

        // Add a dialog to save the graph
        let action = gio::SimpleAction::new("save_as", None);
        let app_weak = self.downgrade();
        action.connect_activate(glib::clone!(@weak window => move |_, _| {
            let app = upgrade_weak!(app_weak);
            let file_chooser = FileChooserDialog::new(
                Some("Save File"),
                Some(&window),
                FileChooserAction::Open,
                &[("Save", ResponseType::Ok), ("Cancel", ResponseType::Cancel)],
            );
            file_chooser.connect_response(move |d: &FileChooserDialog, response: ResponseType| {
                if response == ResponseType::Ok {
                    let file = d.file().expect("Couldn't get file");
                    let filename = String::from(file.path().expect("Couldn't get file path").to_str().expect("unable to convert to string"));
                    println!("Save file {}", filename);
                    app.save_graph(&filename).expect("Unable to save file");
                }

                d.close();
            });

            file_chooser.show();

         }));

        application.add_action(&action);
        application.set_accels_for_action("app.save", &["<primary>s"]);

        let action = gio::SimpleAction::new("quit", None);
        action.connect_activate({
            let app = application.downgrade();
            move |_, _| {
                let app = app.upgrade().unwrap();
                app.quit();
            }
        });
        application.add_action(&action);
        application.set_accels_for_action("app.quit", &["<primary>q"]);

        let action = gio::SimpleAction::new("new-window", None);

        application.add_action(&action);
        application.set_accels_for_action("app.new-window", &["<primary>n"]);

        let about_dialog: AboutDialog = self
            .builder
            .object("dialog-about")
            .expect("Couldn't get window");
        let action = gio::SimpleAction::new("about", None);
        action.connect_activate({
            move |_, _| {
                about_dialog.show();
            }
        });
        application.add_action(&action);
        application.set_accels_for_action("app.about", &["<primary>a"]);

        // Create a dialog to select GStreamer elements.
        let add_button: Button = self
            .builder
            .object("button-add-plugin")
            .expect("Couldn't get app_button");
        let app_weak = self.downgrade();
        add_button.connect_clicked(glib::clone!(@weak window => move |_| {
            // entry.set_text("Clicked!");
            let app = upgrade_weak!(app_weak);
            let elements = Pipeline::elements_list().expect("Unable to obtain element's list");
            pluginlist::display_plugin_list(&app, &elements);
        }));

        let add_button: Button = self
            .builder
            .object("button-play")
            .expect("Couldn't get app_button");
        let app_weak = self.downgrade();
        add_button.connect_clicked(glib::clone!(@weak window => move |_| {
            // entry.set_text("Clicked!");
            let _app = upgrade_weak!(app_weak);

        }));
        let add_button: Button = self
            .builder
            .object("button-stop")
            .expect("Couldn't get app_button");
        let app_weak = self.downgrade();
        add_button.connect_clicked(glib::clone!(@weak window => move |_| {
            let app = upgrade_weak!(app_weak);
            let graph_view = app.graphview.borrow_mut();
            graph_view.remove_all_nodes();
            let node_id = graph_view.get_next_node_id();
            let element_name = String::from("appsink");
            let pads = Pipeline::get_pads(&element_name, false);
            graph_view.add_node_with_port(node_id, Node::new(node_id, &element_name, Pipeline::get_element_type(&element_name)), pads.0, pads.1);
            let node_id = graph_view.get_next_node_id();
            let element_name = String::from("videotestsrc");
            let pads = Pipeline::get_pads(&element_name, false);
            graph_view.add_node_with_port(node_id, Node::new(node_id, &element_name, Pipeline::get_element_type(&element_name)), pads.0, pads.1);
            let node_id = graph_view.get_next_node_id();
            let element_name = String::from("videoconvert");
            let pads = Pipeline::get_pads(&element_name, false);
            graph_view.add_node_with_port(node_id, Node::new(node_id, &element_name, Pipeline::get_element_type(&element_name)), pads.0, pads.1);

        }));
        let add_button: Button = self
            .builder
            .object("button-clear")
            .expect("Couldn't get app_button");
        let app_weak = self.downgrade();
        add_button.connect_clicked(glib::clone!(@weak window => move |_| {
            let app = upgrade_weak!(app_weak);
            let graph_view = app.graphview.borrow_mut();
            graph_view.remove_all_nodes();
        }));
    }

    // Downgrade to a weak reference
    pub fn downgrade(&self) -> GPSAppWeak {
        GPSAppWeak(Rc::downgrade(&self.0))
    }

    // Called when the application shuts down. We drop our app struct here
    fn drop(self) {}

    pub fn add_new_element(&self, element_name: String) {
        let graph_view = self.graphview.borrow_mut();
        let node_id = graph_view.next_node_id();
        let pads = Pipeline::get_pads(&element_name, false);
        graph_view.add_node_with_port(
            node_id,
            Node::new(
                node_id,
                &element_name,
                Pipeline::get_element_type(&element_name),
            ),
            pads.0,
            pads.1,
        );
    }

    pub fn save_graph(&self, filename: &str) -> anyhow::Result<(), Box<dyn error::Error>> {
        let graph_view = self.graphview.borrow_mut();
        graph_view.render_xml(filename)?;
        Ok(())
    }

    pub fn load_graph(&self, filename: &str) -> anyhow::Result<(), Box<dyn error::Error>> {
        let graph_view = self.graphview.borrow_mut();
        graph_view.remove_all_nodes();
        graph_view.load_xml(filename)?;
        Ok(())
    }
}
