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
use glib::Value;
use gtk::gdk::Rectangle;
use gtk::prelude::*;
use gtk::{
    gdk::BUTTON_SECONDARY, AboutDialog, Application, ApplicationWindow, Builder, Button,
    CellRendererText, FileChooserAction, FileChooserDialog, ListStore, PopoverMenu, ResponseType,
    Statusbar, TreeView, TreeViewColumn, Viewport,
};
use gtk::{gio, glib, graphene};
use once_cell::unsync::OnceCell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::{error, ops};

use crate::pipeline::{Pipeline, PipelineState};
use crate::plugindialogs;
use crate::settings::Settings;

use crate::graphmanager::{GraphView, Node, PortDirection};

#[derive(Debug)]
pub struct GPSAppInner {
    pub window: gtk::ApplicationWindow,
    pub graphview: RefCell<GraphView>,
    pub builder: Builder,
    pub pipeline: RefCell<Pipeline>,
    pub plugin_list_initialized: OnceCell<bool>,
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
            plugin_list_initialized: OnceCell::new(),
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
    }

    pub fn get_file_from_dialog<F: Fn(GPSApp, String) + 'static>(app: &GPSApp, save: bool, f: F) {
        let mut message = "Open file";
        let mut ok_button = "Open";
        let cancel_button = "Cancel";
        let mut action = FileChooserAction::Open;
        if save {
            message = "Save file";
            ok_button = "Save";
            action = FileChooserAction::Save;
        }

        let file_chooser: FileChooserDialog = FileChooserDialog::new(
            Some(message),
            Some(&app.window),
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
                        .expect("unable to convert to string"),
                );
                f(app, filename);
            }

            d.close();
        });

        file_chooser.show();
    }

    pub fn show_error_dialog(fatal: bool, message: &str) {
        let app = gio::Application::default()
            .expect("No default application")
            .downcast::<gtk::Application>()
            .expect("Default application has wrong type");

        let dialog = gtk::MessageDialog::new(
            app.active_window().as_ref(),
            gtk::DialogFlags::MODAL,
            gtk::MessageType::Error,
            gtk::ButtonsType::Ok,
            message,
        );

        dialog.connect_response(move |dialog, _| {
            let app = gio::Application::default().expect("No default application");

            dialog.destroy();

            if fatal {
                app.quit();
            }
        });

        dialog.set_resizable(false);
        dialog.show();
    }

    fn reset_favorite_list(&self, favorite_list: &TreeView) {
        let model = ListStore::new(&[String::static_type()]);
        favorite_list.set_model(Some(&model));
        let favorites = Settings::get_favorites_list();
        for favorite in favorites {
            model.insert_with_values(None, &[(0, &favorite)]);
        }
    }

    fn setup_favorite_list(&self) {
        let favorite_list: TreeView = self
            .builder
            .object("favorites_list")
            .expect("Couldn't get window");
        let column = TreeViewColumn::new();
        let cell = CellRendererText::new();

        column.pack_start(&cell, true);
        // Association of the view's column with the model's `id` column.
        column.add_attribute(&cell, "text", 0);
        column.set_title("favorites");
        favorite_list.append_column(&column);
        self.reset_favorite_list(&favorite_list);
        let app_weak = self.downgrade();
        favorite_list.connect_row_activated(move |tree_view, _tree_path, _tree_column| {
            let app = upgrade_weak!(app_weak);
            let selection = tree_view.selection();
            if let Some((model, iter)) = selection.selected() {
                let element_name = model
                    .get(&iter, 0)
                    .get::<String>()
                    .expect("Treeview selection, column 1");
                println!("{}", element_name);
                app.add_new_element(&element_name);
            }
        });
        let gesture = gtk::GestureClick::new();
        gesture.set_button(0);
        let app_weak = self.downgrade();
        gesture.connect_pressed(
            glib::clone!(@weak favorite_list => move |gesture, _n_press, x, y| {
                let app = upgrade_weak!(app_weak);
                if gesture.current_button() == BUTTON_SECONDARY {
                    let selection = favorite_list.selection();
                    if let Some((model, iter)) = selection.selected() {
                        let element_name = model
                        .get(&iter, 0)
                        .get::<String>()
                        .expect("Treeview selection, column 1");
                        println!("{}", element_name);
                        let point = graphene::Point::new(x as f32,y as f32);


                    let pop_menu: PopoverMenu = app
                        .builder
                        .object("fav_pop_menu")
                        .expect("Couldn't get menu model for favorites");

                    pop_menu.set_pointing_to(&Rectangle {
                        x: point.to_vec2().x() as i32,
                        y: point.to_vec2().y() as i32,
                        width: 0,
                        height: 0,
                    });
                    // add an action to delete link
                    let action = gio::SimpleAction::new("favorite.remove", None);
                    let app_weak = app.downgrade();
                    action.connect_activate(glib::clone!(@weak pop_menu => move |_,_| {
                        let app = upgrade_weak!(app_weak);
                        Settings::remove_favorite(&element_name);
                        app.reset_favorite_list(&favorite_list);
                        pop_menu.unparent();
                    }));
                    if let Some(application) = app.window.application() {
                        application.add_action(&action);
                    }

                    pop_menu.show();
                    }

                }
            }),
        );
        favorite_list.add_controller(&gesture);
    }

    fn add_to_favorite_list(&self, element_name: String) {
        let favorites = Settings::get_favorites_list();
        if !favorites.contains(&element_name) {
            let favorite_list: TreeView = self
                .builder
                .object("favorites_list")
                .expect("Couldn't get window");
            if let Some(model) = favorite_list.model() {
                let list_store = model
                    .dynamic_cast::<ListStore>()
                    .expect("Could not cast to ListStore");
                list_store.insert_with_values(None, &[(0, &element_name)]);
                Settings::add_favorite(&element_name);
            }
        }
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
                GPSApp::get_file_from_dialog(&app, false, move |app, filename|
                    {
                        println!("Open file {}", filename);
                        app.load_graph(&filename).expect("Unable to open file");
                    });
        }));
        application.add_action(&action);
        application.set_accels_for_action("app.open", &["<primary>o"]);

        // Add a dialog to save the graph
        let action = gio::SimpleAction::new("save_as", None);
        let app_weak = self.downgrade();
        action.connect_activate(glib::clone!(@weak window => move |_, _| {

           let app = upgrade_weak!(app_weak);
           GPSApp::get_file_from_dialog(&app, true, move |app, filename|
               {
                   println!("Save file {}", filename);
                   app.save_graph(&filename).expect("Unable to save file");
               });
        }));

        application.add_action(&action);
        application.set_accels_for_action("app.save", &["<primary>s"]);

        let action = gio::SimpleAction::new("delete", None);
        application.set_accels_for_action("app.delete", &["<primary>d", "Delete"]);
        let app_weak = self.downgrade();
        action.connect_activate({
            move |_, _| {
                let app = upgrade_weak!(app_weak);
                let graph_view = app.graphview.borrow();
                graph_view.delete_selected();
            }
        });
        application.add_action(&action);

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
        let app_weak = self.downgrade();
        action.connect_activate({
            move |_, _| {
                let app = upgrade_weak!(app_weak);
                app.clear_graph();
            }
        });
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
        //application.set_accels_for_action("app.about", &["<primary>a"]);

        // Create a dialog to select GStreamer elements.
        let add_button: Button = self
            .builder
            .object("button-add-plugin")
            .expect("Couldn't get app_button");
        let app_weak = self.downgrade();
        add_button.connect_clicked(glib::clone!(@weak window => move |_| {
            let app = upgrade_weak!(app_weak);
            let elements = Pipeline::elements_list().expect("Unable to obtain element's list");
            plugindialogs::display_plugin_list(&app, &elements);
        }));

        let add_button: Button = self
            .builder
            .object("button-play")
            .expect("Couldn't get app_button");
        let app_weak = self.downgrade();
        add_button.connect_clicked(glib::clone!(@weak window => move |_| {
            // entry.set_text("Clicked!");
            let app = upgrade_weak!(app_weak);
            let graph_view = app.graphview.borrow();
            let pipeline = app.pipeline.borrow();
            if pipeline.state() == PipelineState::Stopped {
                if let Err(err)  = pipeline.create_pipeline(&pipeline.render_gst_launch(&graph_view)) {
                    GPSApp::show_error_dialog(
                        false,
                        format!("Unable to start a pipeline: {}", err)
                        .as_str(),
                    );
                }
                pipeline.set_state(PipelineState::Playing).expect("Unable to change state");
            } else if pipeline.state() == PipelineState::Paused {
                pipeline.set_state(PipelineState::Playing).expect("Unable to change state");
            } else {
                pipeline.set_state(PipelineState::Paused).expect("Unable to change state");
            }
        }));
        let add_button: Button = self
            .builder
            .object("button-pause")
            .expect("Couldn't get app_button");
        let app_weak = self.downgrade();
        add_button.connect_clicked(glib::clone!(@weak window => move |_| {
            let app = upgrade_weak!(app_weak);
            let graph_view = app.graphview.borrow();
            let pipeline = app.pipeline.borrow();
            if pipeline.state() == PipelineState::Stopped {
                if let Err(err)  = pipeline.create_pipeline(&pipeline.render_gst_launch(&graph_view)) {
                    GPSApp::show_error_dialog(
                        false,
                        format!("Unable to start a pipeline: {}", err)
                        .as_str(),
                    );
                }
                pipeline.set_state(PipelineState::Paused).expect("Unable to change state");
            } else if pipeline.state() == PipelineState::Paused {
                pipeline.set_state(PipelineState::Playing).expect("Unable to change state");
            } else {
                pipeline.set_state(PipelineState::Paused).expect("Unable to change state");
            }
        }));
        let add_button: Button = self
            .builder
            .object("button-stop")
            .expect("Couldn't get app_button");
        let app_weak = self.downgrade();
        add_button.connect_clicked(glib::clone!(@weak window => move |_| {
            let app = upgrade_weak!(app_weak);
            let pipeline = app.pipeline.borrow();
            pipeline.set_state(PipelineState::Stopped).expect("Unable to change state to STOP");
        }));
        let add_button: Button = self
            .builder
            .object("button-clear")
            .expect("Couldn't get app_button");
        let app_weak = self.downgrade();
        add_button.connect_clicked(glib::clone!(@weak window => move |_| {
            let app = upgrade_weak!(app_weak);
            app.load_graph("graphs/video.xml").expect("Unable to open file");
        }));

        let app_weak = self.downgrade();
        // When user clicks on port with right button
        self.graphview
            .borrow()
            .connect_local(
                "port-right-clicked",
                false,
                glib::clone!(@weak application =>  @default-return None, move |values: &[Value]| {
                    let app = upgrade_weak!(app_weak, None);

                    let port_id = values[1].get::<u32>().expect("port id args[1]");
                    let node_id = values[2].get::<u32>().expect("node id args[2]");
                    let point = values[3].get::<graphene::Point>().expect("point in args[3]");

                    let port_menu: gio::MenuModel = app
                        .builder
                        .object("port_menu")
                        .expect("Couldn't get menu model for port");

                    let pop_menu: PopoverMenu = PopoverMenu::from_model(Some(&port_menu));
                    pop_menu.set_parent(&*app.graphview.borrow_mut());
                    pop_menu.set_pointing_to(&Rectangle {
                        x: point.to_vec2().x() as i32,
                        y: point.to_vec2().y() as i32,
                        width: 0,
                        height: 0,
                    });
                    // add an action to delete link
                    let action = gio::SimpleAction::new("port.delete-link", None);
                    action.connect_activate(glib::clone!(@weak pop_menu => move |_,_| {
                        println!("port.delete-link port {} node {}", port_id, node_id);
                        pop_menu.unparent();
                    }));
                    application.add_action(&action);

                    pop_menu.show();
                    None
                }),
            )
            .expect("Failed to register port-right-clicked signal of graphview");

        // When user clicks on  node with right button
        let app_weak = self.downgrade();
        self.graphview
            .borrow()
            .connect_local(
                "node-right-clicked",
                false,
                glib::clone!(@weak application =>  @default-return None, move |values: &[Value]| {
                    let app = upgrade_weak!(app_weak, None);

                    let node_id = values[1].get::<u32>().expect("node id args[1]");
                    let point = values[2].get::<graphene::Point>().expect("point in args[2]");

                    let node_menu: gio::MenuModel = app
                        .builder
                        .object("node_menu")
                        .expect("Couldn't get menu model for node");

                    let pop_menu: PopoverMenu = PopoverMenu::from_model(Some(&node_menu));
                    pop_menu.set_parent(&*app.graphview.borrow_mut());
                    pop_menu.set_pointing_to(&Rectangle {
                        x: point.to_vec2().x() as i32,
                        y: point.to_vec2().y() as i32,
                        width: 0,
                        height: 0,
                    });

                    let action = gio::SimpleAction::new("node.add-to-favorite", None);
                    let app_weak = app.downgrade();
                    action.connect_activate(glib::clone!(@weak pop_menu => move |_,_| {
                        let app = upgrade_weak!(app_weak);
                        println!("node.delete {}", node_id);
                        let node = app.graphview.borrow().node(&node_id).unwrap();
                        app.add_to_favorite_list(node.name());
                        pop_menu.unparent();
                    }));
                    application.add_action(&action);
                    let action = gio::SimpleAction::new("node.delete", None);
                    let app_weak = app.downgrade();
                    action.connect_activate(glib::clone!(@weak pop_menu => move |_,_| {
                        let app = upgrade_weak!(app_weak);
                        println!("node.delete {}", node_id);
                        app.graphview.borrow_mut().remove_node(node_id);
                        pop_menu.unparent();
                    }));
                    application.add_action(&action);

                    let action = gio::SimpleAction::new("node.request-pad-input", None);
                    let app_weak = app.downgrade();
                    action.connect_activate(glib::clone!(@weak pop_menu => move |_,_| {
                        let app = upgrade_weak!(app_weak);
                        println!("node.request-pad-input {}", node_id);
                        let mut node = app.graphview.borrow_mut().node(&node_id).unwrap();
                        let port_id = app.graphview.borrow().next_port_id();
                        node.add_port(port_id, "in", PortDirection::Input);
                        pop_menu.unparent();
                    }));
                    application.add_action(&action);

                    let action = gio::SimpleAction::new("node.request-pad-output", None);
                    let app_weak = app.downgrade();
                    action.connect_activate(glib::clone!(@weak pop_menu => move |_,_| {
                        let app = upgrade_weak!(app_weak);
                        println!("node.request-pad-output {}", node_id);
                        let mut node = app.graphview.borrow_mut().node(&node_id).unwrap();
                        let port_id = app.graphview.borrow_mut().next_port_id();
                        node.add_port(port_id, "out", PortDirection::Output);
                        pop_menu.unparent();
                    }));
                    application.add_action(&action);

                    let action = gio::SimpleAction::new("node.properties", None);
                    action.connect_activate(glib::clone!(@weak pop_menu => move |_,_| {
                        println!("node.properties {}", node_id);
                        let node = app.graphview.borrow().node(&node_id).unwrap();
                        plugindialogs::display_plugin_properties(&app, &node.name(), node_id);
                        pop_menu.unparent();
                    }));
                    application.add_action(&action);


                    pop_menu.show();
                    None
                }),
            )
            .expect("Failed to register node-right-clicked signal of graphview");

        // Setup the favorite list
        self.setup_favorite_list();
    }

    // Downgrade to a weak reference
    pub fn downgrade(&self) -> GPSAppWeak {
        GPSAppWeak(Rc::downgrade(&self.0))
    }

    // Called when the application shuts down. We drop our app struct here
    fn drop(self) {}

    pub fn add_new_element(&self, element_name: &str) {
        let graph_view = self.graphview.borrow_mut();
        let node_id = graph_view.next_node_id();
        let pads = Pipeline::pads(element_name, false);
        if Pipeline::element_is_uri_src_handler(element_name) {
            GPSApp::get_file_from_dialog(self, false, move |app, filename| {
                println!("Open file {}", filename);
                let node = app.graphview.borrow().node(&node_id).unwrap();
                let mut properties: HashMap<String, String> = HashMap::new();
                properties.insert(String::from("location"), filename);
                node.update_node_properties(&properties);
            });
        }
        graph_view.add_node_with_port(
            node_id,
            Node::new(node_id, element_name, Pipeline::element_type(element_name)),
            pads.0,
            pads.1,
        );
    }

    pub fn update_element_properties(&self, node_id: u32, properties: &HashMap<String, String>) {
        let node = self.graphview.borrow().node(&node_id).unwrap();
        node.update_node_properties(properties);
    }

    pub fn clear_graph(&self) {
        let graph_view = self.graphview.borrow_mut();
        graph_view.remove_all_nodes();
    }

    pub fn save_graph(&self, filename: &str) -> anyhow::Result<(), Box<dyn error::Error>> {
        let graph_view = self.graphview.borrow_mut();
        graph_view.render_xml(filename)?;
        Ok(())
    }

    pub fn load_graph(&self, filename: &str) -> anyhow::Result<(), Box<dyn error::Error>> {
        self.clear_graph();
        let graph_view = self.graphview.borrow_mut();
        graph_view.load_xml(filename)?;
        Ok(())
    }
}
