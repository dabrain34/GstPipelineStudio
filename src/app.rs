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

use glib::SignalHandlerId;
use glib::Value;
use gtk::gdk::Rectangle;
use gtk::prelude::*;
use gtk::{
    gdk::BUTTON_SECONDARY, Application, ApplicationWindow, Builder, Button, CellRendererText,
    FileChooserAction, FileChooserDialog, ListStore, Paned, PopoverMenu, ResponseType, Statusbar,
    TreeView, TreeViewColumn, Viewport, Widget,
};
use gtk::{gio, gio::SimpleAction, glib, graphene};
use log::error;
use once_cell::unsync::OnceCell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops;
use std::rc::{Rc, Weak};

use crate::about;
use crate::gps::{ElementInfo, PadInfo, Pipeline, PipelineState};
use crate::logger;
use crate::plugindialogs;
use crate::settings::Settings;
use crate::{GPS_DEBUG, GPS_ERROR, GPS_TRACE, GPS_WARN};

use crate::graphmanager::{GraphView, Node, PortDirection, PortPresence};

#[derive(Debug)]
pub struct GPSAppInner {
    pub window: gtk::ApplicationWindow,
    pub graphview: RefCell<GraphView>,
    pub builder: Builder,
    pub pipeline: RefCell<Pipeline>,
    pub plugin_list_initialized: OnceCell<bool>,
    pub menu_signal_handlers: RefCell<HashMap<String, SignalHandlerId>>,
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
    fn new(application: &gtk::Application) -> anyhow::Result<GPSApp> {
        let glade_src = include_str!("gps.ui");
        let builder = Builder::from_string(glade_src);
        let window: ApplicationWindow = builder
            .object("mainwindow")
            .expect("Couldn't get the main window");
        window.set_application(Some(application));
        window.set_title(Some("GstPipelineStudio"));

        let settings = Settings::load_settings();
        window.set_size_request(settings.app_width, settings.app_height);
        let paned: Paned = builder
            .object("graph_logs-paned")
            .expect("Couldn't get graph_logs-paned");
        paned.set_position(settings.app_graph_logs_paned_pos);
        let paned: Paned = builder
            .object("graph_favorites-paned")
            .expect("Couldn't get graph_favorites-paned");
        paned.set_position(settings.app_graph_favorites_paned_pos);
        if settings.app_maximized {
            window.maximize();
        }
        let pipeline = Pipeline::new().expect("Unable to initialize GStreamer subsystem");
        let app = GPSApp(Rc::new(GPSAppInner {
            window,
            graphview: RefCell::new(GraphView::new()),
            builder,
            pipeline: RefCell::new(pipeline),
            plugin_list_initialized: OnceCell::new(),
            menu_signal_handlers: RefCell::new(HashMap::new()),
        }));
        app.graphview.borrow_mut().set_id(0);
        Ok(app)
    }

    pub fn on_startup(application: &gtk::Application) {
        // Create application and error out if that fails for whatever reason
        let app = match GPSApp::new(application) {
            Ok(app) => app,
            Err(err) => {
                error!("Error creating application: {}", err);
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
            let window: ApplicationWindow = app
                .builder
                .object("mainwindow")
                .expect("Couldn't get the main window");
            let mut settings = Settings::load_settings();
            settings.app_maximized = window.is_maximized();
            settings.app_width = window.width();
            settings.app_height = window.height();
            let paned: Paned = app
                .builder
                .object("graph_logs-paned")
                .expect("Couldn't get graph_logs-paned");
            settings.app_graph_logs_paned_pos = paned.position();
            let paned: Paned = app
                .builder
                .object("graph_favorites-paned")
                .expect("Couldn't get graph_favorites-paned");
            settings.app_graph_favorites_paned_pos = paned.position();
            Settings::save_settings(&settings);

            let pop_menu: PopoverMenu = app
                .builder
                .object("app_pop_menu")
                .expect("Couldn't get app_pop_menu");
            pop_menu.unparent();

            app.drop();
        });
    }

    fn setup_app_actions(&self, application: &gtk::Application) {
        application.add_action(&gio::SimpleAction::new("open", None));
        application.set_accels_for_action("app.open", &["<primary>o"]);

        application.add_action(&gio::SimpleAction::new("save_as", None));
        application.set_accels_for_action("app.save", &["<primary>s"]);

        application.add_action(&gio::SimpleAction::new("delete", None));
        application.set_accels_for_action("app.delete", &["<primary>d", "Delete"]);

        application.add_action(&gio::SimpleAction::new("quit", None));
        application.set_accels_for_action("app.quit", &["<primary>q"]);

        application.add_action(&gio::SimpleAction::new("new-window", None));
        application.set_accels_for_action("app.new-window", &["<primary>n"]);

        application.add_action(&gio::SimpleAction::new("about", None));
        application.set_accels_for_action("app.about", &["<primary>a"]);

        application.add_action(&gio::SimpleAction::new("favorite.remove", None));

        application.add_action(&gio::SimpleAction::new("graph.add-plugin", None));

        application.add_action(&gio::SimpleAction::new("port.delete", None));

        application.add_action(&gio::SimpleAction::new("node.add-to-favorite", None));
        application.add_action(&gio::SimpleAction::new("node.delete", None));
        application.add_action(&gio::SimpleAction::new("node.request-pad-input", None));
        application.add_action(&gio::SimpleAction::new("node.request-pad-output", None));
        application.add_action(&gio::SimpleAction::new("node.properties", None));
    }

    fn app_pop_menu_at_position(&self, widget: &impl IsA<Widget>, x: f64, y: f64) -> PopoverMenu {
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
            pop_menu.set_pointing_to(Some(&Rectangle::new(
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

    fn disconnect_app_menu_action(&self, action_name: &str) {
        let action = self.app_menu_action(action_name);

        if let Some(signal_handler_id) = self.menu_signal_handlers.borrow_mut().remove(action_name)
        {
            action.disconnect(signal_handler_id);
        }
    }

    fn connect_app_menu_action<
        F: Fn(&SimpleAction, std::option::Option<&glib::Variant>) + 'static,
    >(
        &self,
        action_name: &str,
        f: F,
    ) {
        let action = self.app_menu_action(action_name);
        self.disconnect_app_menu_action(action_name);
        let signal_handler_id = action.connect_activate(f);
        self.menu_signal_handlers
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
    fn add_column_to_treeview(&self, tree_name: &str, column_name: &str, column_n: i32) {
        let treeview: TreeView = self
            .builder
            .object(tree_name)
            .expect("Couldn't get tree_name");
        let column = TreeViewColumn::new();
        let cell = CellRendererText::new();
        column.pack_start(&cell, true);
        // Association of the view's column with the model's `id` column.
        column.add_attribute(&cell, "text", column_n);
        column.set_title(column_name);
        treeview.append_column(&column);
    }

    fn reset_logger_list(&self, logger_list: &TreeView) {
        let model = ListStore::new(&[
            String::static_type(),
            String::static_type(),
            String::static_type(),
        ]);
        logger_list.set_model(Some(&model));
    }

    fn setup_logger_list(&self) {
        self.add_column_to_treeview("treeview-logger", "TIME", 0);
        self.add_column_to_treeview("treeview-logger", "LEVEL", 1);
        self.add_column_to_treeview("treeview-logger", "LOG", 2);
        let logger_list: TreeView = self
            .builder
            .object("treeview-logger")
            .expect("Couldn't get treeview-logger");
        self.reset_logger_list(&logger_list);
    }

    fn add_to_logger_list(&self, log_entry: &str) {
        let logger_list: TreeView = self
            .builder
            .object("treeview-logger")
            .expect("Couldn't get treeview-logger");
        if let Some(model) = logger_list.model() {
            let list_store = model
                .dynamic_cast::<ListStore>()
                .expect("Could not cast to ListStore");
            let log: Vec<&str> = log_entry.splitn(3, ' ').collect();
            list_store.insert_with_values(None, &[(0, &log[0]), (1, &log[1]), (2, &log[2])]);
        }
    }

    fn reset_favorite_list(&self, favorite_list: &TreeView) {
        let model = ListStore::new(&[String::static_type()]);
        favorite_list.set_model(Some(&model));
        let favorites = Settings::get_favorites_list();
        for favorite in favorites {
            model.insert_with_values(None, &[(0, &favorite)]);
        }
    }

    fn setup_favorite_list(&self, application: &Application) {
        let favorite_list: TreeView = self
            .builder
            .object("treeview-favorites")
            .expect("Couldn't get treeview-favorites");
        self.add_column_to_treeview("treeview-favorites", "Element", 0);
        self.reset_favorite_list(&favorite_list);
        let app_weak = self.downgrade();
        favorite_list.connect_row_activated(move |tree_view, _tree_path, _tree_column| {
            let app = upgrade_weak!(app_weak);
            let selection = tree_view.selection();
            if let Some((model, iter)) = selection.selected() {
                let element_name = model.get::<String>(&iter, 1);
                GPS_DEBUG!("{} selected", element_name);
                app.add_new_element(&element_name);
            }
        });
        let gesture = gtk::GestureClick::new();
        gesture.set_button(0);
        let app_weak = self.downgrade();
        gesture.connect_pressed(
            glib::clone!(@weak favorite_list, @weak application => move |gesture, _n_press, x, y| {
                let app = upgrade_weak!(app_weak);
                if gesture.current_button() == BUTTON_SECONDARY {
                    let selection = favorite_list.selection();
                    if let Some((model, iter)) = selection.selected() {
                        let element_name = model
                        .get::<String>(&iter, 1);
                        GPS_DEBUG!("Element {} selected", element_name);

                        let pop_menu = app.app_pop_menu_at_position(&favorite_list, x, y);
                        let menu: gio::MenuModel = app
                        .builder
                        .object("fav_menu")
                        .expect("Couldn't get fav_menu model");
                        pop_menu.set_menu_model(Some(&menu));

                        let app_weak = app.downgrade();
                        app.connect_app_menu_action("favorite.remove",
                            move |_,_| {
                                let app = upgrade_weak!(app_weak);
                                Settings::remove_favorite(&element_name);
                                app.reset_favorite_list(&favorite_list);
                            }
                        );

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
                .object("treeview-favorites")
                .expect("Couldn't get treeview-favorites");
            if let Some(model) = favorite_list.model() {
                let list_store = model
                    .dynamic_cast::<ListStore>()
                    .expect("Could not cast to ListStore");
                list_store.insert_with_values(None, &[(0, &element_name)]);
                Settings::add_favorite(&element_name);
            }
        }
    }

    pub fn display_plugin_list(app: &GPSApp) {
        let elements = ElementInfo::elements_list().expect("Unable to obtain element's list");
        plugindialogs::display_plugin_list(app, &elements);
    }

    pub fn build_ui(&self, application: &Application) {
        let drawing_area_window: Viewport = self
            .builder
            .object("drawing_area")
            .expect("Couldn't get drawing_area");

        drawing_area_window.set_child(Some(&*self.graphview.borrow()));

        // Setup the logger to get messages into the TreeView
        let (ready_tx, ready_rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let app_weak = self.downgrade();
        logger::init_logger(
            ready_tx,
            Settings::default_log_file_path()
                .to_str()
                .expect("Unable to convert log file path to a string"),
        );
        self.setup_logger_list();
        let _ = ready_rx.attach(None, move |msg: String| {
            let app = upgrade_weak!(app_weak, glib::Continue(false));
            app.add_to_logger_list(&msg);
            glib::Continue(true)
        });

        let window = &self.window;

        window.show();
        let status_bar: Statusbar = self
            .builder
            .object("status_bar")
            .expect("Couldn't get status_bar");
        status_bar.push(status_bar.context_id("Description"), "GPS is ready");

        self.setup_app_actions(application);

        let pop_menu: PopoverMenu = self
            .builder
            .object("app_pop_menu")
            .expect("Couldn't get app_pop_menu");
        pop_menu.set_parent(window);

        let app_weak = self.downgrade();
        self.connect_app_menu_action("new-window", move |_, _| {
            let app = upgrade_weak!(app_weak);
            app.clear_graph();
            GPS_ERROR!("clear graph");
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("open", move |_, _| {
            let app = upgrade_weak!(app_weak);
            GPSApp::get_file_from_dialog(&app, false, move |app, filename| {
                app.load_graph(&filename)
                    .unwrap_or_else(|_| GPS_ERROR!("Unable to open file {}", filename));
            });
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("save_as", move |_, _| {
            let app = upgrade_weak!(app_weak);
            GPSApp::get_file_from_dialog(&app, true, move |app, filename| {
                GPS_DEBUG!("Save file {}", filename);
                app.save_graph(&filename)
                    .unwrap_or_else(|_| GPS_ERROR!("Unable to save file to {}", filename));
            });
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("delete", move |_, _| {
            let app = upgrade_weak!(app_weak);
            let graph_view = app.graphview.borrow();
            graph_view.delete_selected();
        });

        let app = application.downgrade();
        self.connect_app_menu_action("quit", move |_, _| {
            let app = app.upgrade().unwrap();
            app.quit();
        });

        let app_weak = self.downgrade();
        self.connect_app_menu_action("about", move |_, _| {
            let app = upgrade_weak!(app_weak);
            about::display_about_dialog(&app);
        });

        let app_weak = self.downgrade();
        self.connect_button_action("button-add-plugin", move |_| {
            let app = upgrade_weak!(app_weak);
            GPSApp::display_plugin_list(&app);
        });

        let app_weak = self.downgrade();
        self.connect_button_action("button-play", move |_| {
            let app = upgrade_weak!(app_weak);
            let graph_view = app.graphview.borrow();
            let _ = app
                .pipeline
                .borrow()
                .start_pipeline(&graph_view, PipelineState::Playing);
        });

        let app_weak = self.downgrade();
        self.connect_button_action("button-pause", move |_| {
            let app = upgrade_weak!(app_weak);
            let graph_view = app.graphview.borrow();
            let _ = app
                .pipeline
                .borrow()
                .start_pipeline(&graph_view, PipelineState::Paused);
        });

        let app_weak = self.downgrade();
        self.connect_button_action("button-stop", move |_| {
            let app = upgrade_weak!(app_weak);
            let pipeline = app.pipeline.borrow();
            let _ = pipeline.set_state(PipelineState::Stopped);
        });

        let app_weak = self.downgrade();
        self.connect_button_action("button-clear", move |_| {
            let app = upgrade_weak!(app_weak);
            app.clear_graph();
        });

        let app_weak = self.downgrade();
        self.graphview.borrow().connect_local(
            "graph-updated",
            false,
            glib::clone!(@weak application =>  @default-return None, move |values: &[Value]| {
                let app = upgrade_weak!(app_weak, None);
                let id = values[1].get::<u32>().expect("id in args[1]");
                GPS_TRACE!("Graph updated id={}", id);
                let _ = app
                    .save_graph(
                        Settings::default_graph_file_path()
                            .to_str()
                            .expect("Unable to convert to string"),
                    )
                    .map_err(|e| GPS_WARN!("Unable to save file {}", e));
                None
            }),
        );
        // When user clicks on port with right button
        let app_weak = self.downgrade();
        self.graphview
            .borrow()
            .connect_local(
                "graph-right-clicked",
                false,
                glib::clone!(@weak application =>  @default-return None, move |values: &[Value]| {
                    let app = upgrade_weak!(app_weak, None);
                    let point = values[1].get::<graphene::Point>().expect("point in args[2]");

                    let pop_menu = app.app_pop_menu_at_position(&*app.graphview.borrow(), point.to_vec2().x() as f64, point.to_vec2().y() as f64);
                    let menu: gio::MenuModel = app
                    .builder
                    .object("graph_menu")
                    .expect("Couldn't graph_menu");
                    pop_menu.set_menu_model(Some(&menu));

                    let app_weak = app.downgrade();
                    app.connect_app_menu_action("graph.add-plugin",
                        move |_,_| {
                            let app = upgrade_weak!(app_weak);
                            GPSApp::display_plugin_list(&app);
                        }
                    );
                    pop_menu.show();
                    None
                }),
            );

        // When user clicks on port with right button
        let app_weak = self.downgrade();
        self.graphview.borrow().connect_local(
            "port-right-clicked",
            false,
            move |values: &[Value]| {
                let app = upgrade_weak!(app_weak, None);

                let port_id = values[1].get::<u32>().expect("port id args[1]");
                let node_id = values[2].get::<u32>().expect("node id args[2]");
                let point = values[3]
                    .get::<graphene::Point>()
                    .expect("point in args[3]");

                let pop_menu = app.app_pop_menu_at_position(
                    &*app.graphview.borrow(),
                    point.to_vec2().x() as f64,
                    point.to_vec2().y() as f64,
                );
                let menu: gio::MenuModel = app
                    .builder
                    .object("port_menu")
                    .expect("Couldn't get menu model for port");
                pop_menu.set_menu_model(Some(&menu));

                if app.graphview.borrow().can_remove_port(node_id, port_id) {
                    let app_weak = app.downgrade();
                    app.connect_app_menu_action("port.delete", move |_, _| {
                        let app = upgrade_weak!(app_weak);
                        GPS_DEBUG!("port.delete-link port {} node {}", port_id, node_id);
                        app.graphview.borrow().remove_port(node_id, port_id);
                    });
                } else {
                    app.disconnect_app_menu_action("port.delete");
                }

                pop_menu.show();
                None
            },
        );

        // When user clicks on node with right button
        let app_weak = self.downgrade();
        self.graphview
            .borrow()
            .connect_local(
                "node-right-clicked",
                false,
                glib::clone!(@weak application =>  @default-return None, move |values: &[Value]| {
                    let app = upgrade_weak!(app_weak, None);

                    let node_id = values[1].get::<u32>().expect("node id args[1]");
                    let node = app.graphview.borrow().node(node_id).expect("Unable to find node with this ID");
                    let point = values[2].get::<graphene::Point>().expect("point in args[2]");

                    let pop_menu = app.app_pop_menu_at_position(&*app.graphview.borrow(), point.to_vec2().x() as f64, point.to_vec2().y() as f64);
                    let menu: gio::MenuModel = app
                        .builder
                        .object("node_menu")
                        .expect("Couldn't get menu model for node");
                    pop_menu.set_menu_model(Some(&menu));

                    let app_weak = app.downgrade();
                    app.connect_app_menu_action("node.add-to-favorite",
                        move |_,_| {
                            let app = upgrade_weak!(app_weak);
                            GPS_DEBUG!("node.add-to-favorite {}", node_id);
                            if let Some(node) = app.graphview.borrow().node(node_id) {
                                app.add_to_favorite_list(node.name());
                            };
                        }
                    );

                    let app_weak = app.downgrade();
                    app.connect_app_menu_action("node.delete",
                        move |_,_| {
                            let app = upgrade_weak!(app_weak);
                            GPS_DEBUG!("node.delete {}", node_id);
                            app.graphview.borrow_mut().remove_node(node_id);
                        }
                    );

                    if ElementInfo::element_supports_new_pad_request(&node.name(), PortDirection::Input) {
                        let app_weak = app.downgrade();
                        app.connect_app_menu_action("node.request-pad-input",
                            move |_,_| {
                                let app = upgrade_weak!(app_weak);
                                GPS_DEBUG!("node.request-pad-input {}", node_id);
                                let port_id = app.graphview.borrow().next_port_id();
                                app.graphview.borrow().add_port(node_id, port_id, "in", PortDirection::Input, PortPresence::Sometimes);
                            }
                        );
                    } else {
                        app.disconnect_app_menu_action("node.request-pad-input");
                    }
                    if ElementInfo::element_supports_new_pad_request(&node.name(), PortDirection::Output) {
                    let app_weak = app.downgrade();
                    app.connect_app_menu_action("node.request-pad-output",
                        move |_,_| {
                            let app = upgrade_weak!(app_weak);
                            GPS_DEBUG!("node.request-pad-output {}", node_id);
                            let port_id = app.graphview.borrow_mut().next_port_id();
                            app.graphview.borrow().add_port(node_id, port_id, "out", PortDirection::Output, PortPresence::Sometimes);

                        }
                    );
                    } else {
                        app.disconnect_app_menu_action("node.request-pad-output");
                    }

                    let app_weak = app.downgrade();
                    app.connect_app_menu_action("node.properties",
                        move |_,_| {
                            let app = upgrade_weak!(app_weak);
                            GPS_DEBUG!("node.properties {}", node_id);
                            let node = app.graphview.borrow().node(node_id).unwrap();
                            plugindialogs::display_plugin_properties(&app, &node.name(), node_id);
                        }
                    );

                    pop_menu.show();
                    None
                }),
            );

        // Setup the favorite list
        self.setup_favorite_list(application);

        let _ = self
            .load_graph(
                Settings::default_graph_file_path()
                    .to_str()
                    .expect("Unable to convert to string"),
            )
            .map_err(|_e| {
                GPS_WARN!("Unable to load default graph");
            });
    }

    // Downgrade to a weak reference
    pub fn downgrade(&self) -> GPSAppWeak {
        GPSAppWeak(Rc::downgrade(&self.0))
    }

    // Called when the application shuts down. We drop our app struct here
    fn drop(self) {}

    pub fn add_new_element(&self, element_name: &str) {
        let graph_view = self.graphview.borrow();
        let node_id = graph_view.next_node_id();
        let (inputs, outputs) = PadInfo::pads(element_name, false);
        if ElementInfo::element_is_uri_src_handler(element_name) {
            GPSApp::get_file_from_dialog(self, false, move |app, filename| {
                GPS_DEBUG!("Open file {}", filename);
                let node = app.graphview.borrow().node(node_id).unwrap();
                let mut properties: HashMap<String, String> = HashMap::new();
                properties.insert(String::from("location"), filename);
                node.update_properties(&properties);
            });
        }
        graph_view.add_node_with_port(
            node_id,
            Node::new(
                node_id,
                element_name,
                ElementInfo::element_type(element_name),
            ),
            inputs.len() as u32,
            outputs.len() as u32,
        );
    }

    pub fn update_element_properties(&self, node_id: u32, properties: &HashMap<String, String>) {
        let node = self.graphview.borrow().node(node_id).unwrap();
        node.update_properties(properties);
    }

    fn clear_graph(&self) {
        let graph_view = self.graphview.borrow_mut();
        graph_view.remove_all_nodes();
    }

    fn save_graph(&self, filename: &str) -> anyhow::Result<()> {
        let graph_view = self.graphview.borrow();
        graph_view.render_xml(filename)?;
        Ok(())
    }

    fn load_graph(&self, filename: &str) -> anyhow::Result<()> {
        self.clear_graph();
        let graph_view = self.graphview.borrow();
        graph_view.load_xml(filename)?;
        Ok(())
    }
}
