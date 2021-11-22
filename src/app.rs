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
use gtk::cairo::Context;
use gtk::glib;
use gtk::prelude::*;
use gtk::{
    AboutDialog, AccelFlags, AccelGroup, ApplicationWindow, Builder, Button, DrawingArea, EventBox,
    FileChooserDialog, MenuItem, ResponseType, Viewport, WindowPosition,
};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::{error, ops};

use crate::graph::{Element, Graph};
use crate::pipeline::Pipeline;
use crate::pluginlist;

#[derive(Debug)]
pub struct GPSAppInner {
    pub window: gtk::ApplicationWindow,
    pub drawing_area: DrawingArea,
    pub builder: Builder,
    pub pipeline: RefCell<Pipeline>,
    pub graph: RefCell<Graph>,
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

fn draw_elements(elements: &Vec<Element>, c: &Context) {
    for element in elements {
        c.rectangle(element.position.0, element.position.1, 80.0, 45.0);
        c.fill().expect("Can not draw into context");
    }
}

impl GPSApp {
    fn new(application: &gtk::Application) -> anyhow::Result<GPSApp, Box<dyn error::Error>> {
        let glade_src = include_str!("gps.ui");
        let builder = Builder::from_string(glade_src);
        let window: ApplicationWindow = builder.object("mainwindow").expect("Couldn't get window");
        window.set_application(Some(application));
        window.set_title("GstPipelineStudio");
        window.set_position(WindowPosition::Center);
        window.set_size_request(800, 600);
        let pipeline = Pipeline::new().expect("Unable to initialize the pipeline");
        let drawing_area = DrawingArea::new();
        let app = GPSApp(Rc::new(GPSAppInner {
            window,
            drawing_area,
            builder,
            pipeline: RefCell::new(pipeline),
            graph: RefCell::new(Graph::default()),
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
        application.connect_activate(move |_| {
            let app = upgrade_weak!(app_weak);
            app.build_ui();
        });

        let app_container = RefCell::new(Some(app));
        application.connect_shutdown(move |_| {
            let app = app_container
                .borrow_mut()
                .take()
                .expect("Shutdown called multiple times");
            app.drop();
        });
    }

    pub fn build_ui(&self) {
        let view_port: Viewport = self
            .builder
            .object("drawing_area")
            .expect("Couldn't get window");
        let event_box = EventBox::new();
        event_box.add(&self.drawing_area);
        view_port.add(&event_box);

        let app_weak = self.downgrade();
        self.drawing_area.connect_draw(move |w, c| {
            let app = upgrade_weak!(app_weak, gtk::Inhibit(false));
            println!("w: {} c:{}", w, c);
            let mut graph = app.graph.borrow_mut();
            let elements = graph.elements();
            draw_elements(&elements, c);
            gtk::Inhibit(false)
        });
        let app_weak = self.downgrade();
        event_box.connect_button_release_event(move |_w, evt| {
            let app = upgrade_weak!(app_weak, gtk::Inhibit(false));
            let mut element: Element = Default::default();
            element.position.0 = evt.position().0;
            element.position.1 = evt.position().1;
            app.add_new_element(element);
            app.drawing_area.queue_draw();
            gtk::Inhibit(false)
        });
        let window = &self.window;

        window.show_all();

        let quit: MenuItem = self
            .builder
            .object("menu-quit")
            .expect("Couldn't get window");
        let about: MenuItem = self
            .builder
            .object("menu-about")
            .expect("Couldn't get window");
        let about_dialog: AboutDialog = self
            .builder
            .object("dialog-about")
            .expect("Couldn't get window");
        about.connect_activate(move |_| {
            about_dialog.connect_delete_event(|dialog, _| {
                dialog.hide();
                gtk::Inhibit(true)
            });

            about_dialog.show_all();
        });

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
        // Create a dialog to open a file
        let open_button: Button = self
            .builder
            .object("button-open-file")
            .expect("Couldn't get app_button");
        let open_dialog: FileChooserDialog = self
            .builder
            .object("dialog-open-file")
            .expect("Couldn't get window");
        open_button.connect_clicked(glib::clone!(@weak window => move |_| {
            // entry.set_text("Clicked!");
            open_dialog.connect_response(|dialog, _| dialog.close());
            open_dialog.add_buttons(&[
                ("Open", ResponseType::Ok),
                ("Cancel", ResponseType::Cancel)
            ]);
            open_dialog.set_select_multiple(true);
            open_dialog.connect_response(|open_dialog, response| {
                if response == ResponseType::Ok {
                    let files = open_dialog.filenames();
                    println!("Files: {:?}", files);
                }
                open_dialog.close();
            });
            open_dialog.show_all();
        }));

        let accel_group = AccelGroup::new();
        window.add_accel_group(&accel_group);

        quit.connect_activate(glib::clone!(@weak window => move |_| {
            window.close();
        }));

        // `Primary` is `Ctrl` on Windows and Linux, and `command` on macOS
        // It isn't available directly through `gdk::ModifierType`, since it has
        // different values on different platforms.
        let (key, modifier) = gtk::accelerator_parse("<Primary>Q");
        quit.add_accelerator("activate", &accel_group, key, modifier, AccelFlags::VISIBLE);
    }

    // Downgrade to a weak reference
    pub fn downgrade(&self) -> GPSAppWeak {
        GPSAppWeak(Rc::downgrade(&self.0))
    }

    // Called when the application shuts down. We drop our app struct here
    fn drop(self) {}

    pub fn add_new_element(&self, element: Element) {
        self.graph.borrow_mut().add_element(element);
        self.drawing_area.queue_draw();
    }
}
