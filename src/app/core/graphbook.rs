// graphbook.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

//! Graph tab management and serialization.
//!
//! Manages the notebook containing multiple graph tabs, each representing a separate
//! pipeline workspace. Handles tab lifecycle, XML serialization/deserialization,
//! and comprehensive event handling for graph interactions (node/port/link events).
//! Includes automatic static pad restoration and caps validation.

use glib::Value;
use gtk::glib;
use gtk::prelude::*;
use gtk::{gio, graphene};
use std::cell::{Cell, Ref, RefCell};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::gps as GPS;
use crate::graphmanager as GM;
use crate::graphmanager::PropertyExt;
use crate::logger;
use crate::ui as GPSUI;
use crate::ui::resources::GRAPHVIEW_THEME_CSS;
use crate::{GPS_DEBUG, GPS_ERROR, GPS_TRACE, GPS_WARN};

use super::super::settings::Settings;
use super::super::{GPSApp, GPSAppWeak};

/// Creates a link between two ports, automatically handling direction.
///
/// Given a source port and target port, this function determines the correct
/// node ordering based on the source port's direction (output ports connect
/// from→to, input ports connect to→from).
fn create_directional_link(
    app: &GPSApp,
    from_node_id: u32,
    from_port_id: u32,
    from_port_direction: GM::PortDirection,
    target_node_id: u32,
    target_port_id: u32,
) {
    if from_port_direction == GM::PortDirection::Output {
        app.create_link(from_node_id, target_node_id, from_port_id, target_port_id);
    } else {
        app.create_link(target_node_id, from_node_id, target_port_id, from_port_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum TabState {
    #[default]
    Undefined = 0,
    Modified,
    Saved,
}

#[derive(Debug, Clone, Default)]
pub struct GraphTab {
    graphview: RefCell<GM::GraphView>,
    player: RefCell<GPS::Player>,
    id: Cell<u32>,
    name: gtk::Label,
    filename: RefCell<String>,
    state: Cell<TabState>,
}

impl GraphTab {
    /// Sanitize filename to prevent path traversal and other security issues.
    ///
    /// Returns the sanitized filename, or the original if it's empty.
    fn sanitize_filename(filename: &str) -> String {
        // If filename is empty, return as-is (will be "Untitled")
        if filename.is_empty() {
            return filename.to_string();
        }

        // Extract just the filename component, removing any directory path
        let path = Path::new(filename);
        let sanitized = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled");

        // Additional check: reject filenames with path traversal sequences
        if sanitized.contains("..") || sanitized.contains('/') || sanitized.contains('\\') {
            GPS_WARN!("Rejecting suspicious filename: {}", filename);
            return String::from("Untitled");
        }

        sanitized.to_string()
    }

    pub fn new(app: GPSAppWeak, id: u32, filename: &str) -> Self {
        let label = gtk::Label::new(Some("Untitled*"));

        // Sanitize filename to prevent path traversal attacks
        let sanitized_filename = Self::sanitize_filename(filename);

        let graphtab = GraphTab {
            id: Cell::new(id),
            graphview: RefCell::new(GM::GraphView::new()),
            player: RefCell::new(
                GPS::Player::new().expect("Unable to initialize GStreamer subsystem"),
            ),
            name: label,
            filename: RefCell::new(sanitized_filename),
            state: Cell::new(TabState::Undefined),
        };
        graphtab
            .graphview
            .borrow()
            .set_id(graphbook_get_new_graphview_id(&app));

        // Apply saved dark theme setting
        graphtab
            .graphview
            .borrow()
            .set_dark_theme(Settings::dark_theme());

        // Apply custom graphview theme CSS from app
        graphtab
            .graphview
            .borrow()
            .set_custom_css(GRAPHVIEW_THEME_CSS);

        if let Err(e) = graphtab.player.borrow().set_app(app) {
            GPS_ERROR!("Failed to set app on player: {}", e);
        }
        graphtab
    }

    pub fn id(&self) -> u32 {
        self.id.get()
    }

    pub fn widget_label(&self) -> &gtk::Label {
        &self.name
    }

    pub fn graphview(&self) -> Ref<'_, GM::GraphView> {
        self.graphview.borrow()
    }

    pub fn player(&self) -> Ref<'_, GPS::Player> {
        self.player.borrow()
    }

    pub fn set_name(&self, name: &str) {
        self.name.set_text(name);
    }

    pub fn basename(&self) -> String {
        Path::new(&self.filename.borrow().as_str())
            .file_name()
            .and_then(|f| f.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Untitled".to_string())
    }

    pub fn set_filename(&self, filename: &str) {
        self.filename.replace(filename.to_string());
        self.set_name(self.basename().as_str());
        self.set_modified(false);
    }

    pub fn filename(&self) -> String {
        self.filename.borrow().clone()
    }

    pub fn set_modified(&self, modified: bool) {
        let name = self.basename();
        if modified {
            self.set_name(&(name + "*"));
            self.state.set(TabState::Modified);
        } else {
            self.set_name(name.as_str());
            self.state.set(TabState::Saved);
        }
    }

    pub fn undefined(&self) -> bool {
        self.state.get() == TabState::Undefined
    }

    pub fn modified(&self) -> bool {
        self.state.get() == TabState::Modified
    }
}

pub fn graphtab(app: &GPSApp, id: u32) -> GraphTab {
    app.graphbook
        .borrow()
        .get(&id)
        .cloned()
        .expect("the current graphtab should be ok")
}

pub fn graphbook_get_new_graphview_id(app_weak: &GPSAppWeak) -> u32 {
    let app = app_weak.upgrade();
    let mut graphview_id: u32 = 0;
    for tab in app.unwrap().graphbook.borrow().values() {
        if tab.graphview().id() > graphview_id {
            graphview_id = tab.graphview().id()
        }
    }
    graphview_id + 1
}

pub fn graphbook_get_new_graphtab_id(app: &GPSApp) -> u32 {
    let mut graphtab_id: u32 = 0;
    for tab in app.graphbook.borrow().values() {
        if tab.id() > graphtab_id {
            graphtab_id = tab.id()
        }
    }
    graphtab_id + 1
}

pub fn current_graphtab(app: &GPSApp) -> GraphTab {
    graphtab(app, app.current_graphtab.get())
}

pub fn current_graphtab_set_filename(app: &GPSApp, filename: &str) {
    app.graphbook
        .borrow()
        .get(&app.current_graphtab.get())
        .expect("the graphtab is available")
        .set_filename(filename);
}

pub fn current_graphtab_set_modified(app: &GPSApp, modified: bool) {
    app.graphbook
        .borrow()
        .get(&app.current_graphtab.get())
        .expect("the graphtab is available")
        .set_modified(modified);
}

pub fn setup_graphbook(app: &GPSApp) {
    let graphbook: gtk::Notebook = app
        .builder
        .object("graphbook")
        .expect("Couldn't get the graphbook");
    let app_weak = app.downgrade();
    graphbook.connect_switch_page(move |_book, widget, page| {
        let graphview = widget
            .first_child()
            .expect("Unable to get the child from the graphbook, ie the scrolledWindow");
        if let Ok(graphview) = graphview.dynamic_cast::<GM::GraphView>() {
            let app = upgrade_weak!(app_weak);
            GPS_TRACE!("graphview.id() {} graphbook page {}", graphview.id(), page);
            app.current_graphtab.set(page);
        }
    });
}

pub fn create_graphtab(app: &GPSApp, id: u32, name: Option<&str>) {
    let graph_tab = GraphTab::new(app.downgrade(), id, name.unwrap_or("Untitled"));
    let gt = graph_tab.clone();
    app.graphbook.borrow_mut().insert(id, graph_tab);

    let graphbook: gtk::Notebook = app
        .builder
        .object("graphbook")
        .expect("Couldn't get graphbook");

    let scrollwindow = gtk::ScrolledWindow::builder()
        .name("graphview_scroll")
        .child(&*graphtab(app, id).graphview())
        .build();

    let tab_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let label = gt.widget_label();
    tab_box.append(label);
    let icon = gtk::Image::from_icon_name("window-close-symbolic");
    let close_button = gtk::Button::builder().build();
    close_button.set_child(Some(&icon));
    close_button.add_css_class("small-button");
    close_button.add_css_class("image-button");
    close_button.add_css_class("flat");
    let app_weak = app.downgrade();
    close_button.connect_clicked(glib::clone!(
        #[weak]
        graphbook,
        move |_| {
            let app = upgrade_weak!(app_weak);
            graphbook.remove_page(Some(current_graphtab(&app).id()));
        }
    ));
    tab_box.append(&close_button);
    graphbook.append_page(&scrollwindow, Some(&tab_box));
    graphbook.set_tab_reorderable(&scrollwindow, true);

    // Handle node link requests from graphview.
    //
    // Signal flow and ordering:
    // 1. User clicks port -> port becomes selected
    // 2. User clicks different node -> GraphView emits "node-link-request"
    // 3. This handler calls handle_auto_connect() which validates caps and creates link
    // 4. If a link is created, GraphView emits "link-added" (processed synchronously)
    // 5. The link-added handler performs a second caps check as defense-in-depth
    // 6. We check here if the link was rejected and clean up any orphaned request ports
    //
    // Note: GTK signal handlers within the same main loop iteration are processed
    // synchronously, so by the time we check port_is_linked(), the link-added
    // handler has already run and potentially removed incompatible links.
    let app_weak = app.downgrade();
    gt.graphview().connect_local(
        "node-link-request",
        false,
        glib::clone!(move |values: &[Value]| {
            let app = upgrade_weak!(app_weak, None);

            // Get the graphview that emitted the signal (safer than current_graphtab for multi-tab scenarios)
            let graphview = values[0]
                .get::<GM::GraphView>()
                .expect("signal emitter should be GraphView");

            let from_node_id = values[1].get::<u32>().expect("from_node_id args[1]");
            let from_port_id = values[2].get::<u32>().expect("from_port_id args[2]");
            let target_node_id = values[3].get::<u32>().expect("target_node_id args[3]");

            GPS_TRACE!(
                "Node link request: node {} port {} to node {}",
                from_node_id,
                from_port_id,
                target_node_id
            );

            // Attempt auto-connect and handle cleanup for newly created ports
            if let Some((node_id, port_id)) =
                app.handle_auto_connect(&graphview, from_node_id, from_port_id, target_node_id)
            {
                // Check if the newly created port was successfully linked.
                // Since handle_auto_connect() validates caps before creating links,
                // link rejection here would only happen due to defense-in-depth checks
                // in the link-added handler (e.g., for edge cases not caught earlier).
                if graphview.port_is_linked(port_id).is_some() {
                    GPS_DEBUG!(
                        "Auto-connect succeeded: port {} on node {} is now linked",
                        port_id,
                        node_id
                    );
                } else {
                    // Link was rejected - remove the orphaned request port
                    GPS_DEBUG!(
                        "Auto-connect failed: removing orphaned port {} from node {}",
                        port_id,
                        node_id
                    );
                    graphview.remove_port(node_id, port_id);
                }
            }

            None
        }),
    );

    let app_weak = app.downgrade();
    gt.graphview().connect_local(
        "graph-updated",
        false,
        glib::clone!(move |values: &[Value]| {
            let app = upgrade_weak!(app_weak, None);
            let id = values[1].get::<u32>().expect("id in args[1]");
            GPS_DEBUG!("Graph updated id={}", id);
            let _ = app
                .save_graph(
                    Settings::graph_file_path()
                        .to_str()
                        .expect("Unable to convert to string"),
                )
                .map_err(|e| GPS_WARN!("Unable to save file {}", e));
            current_graphtab_set_modified(&app, true);
            None
        }),
    );
    let app_weak = app.downgrade();
    gt.graphview().connect_local(
        "node-added",
        false,
        glib::clone!(move |values: &[Value]| {
            let app = upgrade_weak!(app_weak, None);
            let graph_id = values[1].get::<u32>().expect("graph id in args[1]");
            let node_id = values[2].get::<u32>().expect("node id in args[2]");
            GPS_DEBUG!("Node added node id={} in graph id={}", node_id, graph_id);
            if let Some(node) = current_graphtab(&app).graphview().node(node_id) {
                let description = GPS::ElementInfo::element_description(&node.name()).ok();
                node.set_tooltip_markup(description.as_deref());
                if !GPS::ElementInfo::element_factory_exists(&node.name()) {
                    node.set_light(true);
                }
                for port in node.all_ports(GM::PortDirection::All) {
                    let caps = PropertyExt::property(&port, "_caps");
                    GPS_DEBUG!(
                        "caps={} for port id {}",
                        caps.clone().unwrap_or_else(|| "caps unknown".to_string()),
                        port.id()
                    );
                    let tooltip = format!(
                        "<b>{}</b>\n{}",
                        port.name(),
                        caps.unwrap_or_else(|| "caps unknown".to_string())
                    );
                    port.set_tooltip_markup(Some(&tooltip));
                }
            }

            None
        }),
    );
    let app_weak = app.downgrade();
    gt.graphview().connect_local(
        "port-added",
        false,
        glib::clone!(move |values: &[Value]| {
            let app = upgrade_weak!(app_weak, None);
            let graph_id = values[1].get::<u32>().expect("graph id in args[1]");
            let node_id = values[2].get::<u32>().expect("node id in args[2]");
            let port_id = values[3].get::<u32>().expect("port id in args[3]");
            GPS_DEBUG!(
                "Port added port id={} to node id={} in graph id={}",
                port_id,
                node_id,
                graph_id
            );
            if let Some(node) = current_graphtab(&app).graphview().node(node_id) {
                if let Some(port) = node.port(port_id) {
                    let caps = PropertyExt::property(&port, "_caps");
                    GPS_DEBUG!(
                        "caps={} for port id {}",
                        caps.clone().unwrap_or_else(|| "caps unknown".to_string()),
                        port.id()
                    );
                    let tooltip = format!(
                        "<b>{}</b>\n{}",
                        port.name(),
                        caps.unwrap_or_else(|| "caps unknown".to_string())
                    );
                    port.set_tooltip_markup(Some(&tooltip));
                }
            }
            None
        }),
    );
    // When user clicks on port with right button
    let app_weak = app.downgrade();
    gt.graphview().connect_local(
        "graph-right-clicked",
        false,
        glib::clone!(move |values: &[Value]| {
            let app = upgrade_weak!(app_weak, None);
            let point = values[1]
                .get::<graphene::Point>()
                .expect("point in args[2]");
            let menu: gio::MenuModel = app
                .builder
                .object("graph_menu")
                .expect("Couldn't graph_menu");
            let app_weak = app.downgrade();
            app.connect_app_menu_action("graph.clear", move |_, _| {
                let app = upgrade_weak!(app_weak);
                current_graphtab(&app).graphview().clear();
            });
            let app_weak = app.downgrade();
            app.connect_app_menu_action("graph.check", move |_, _| {
                let app = upgrade_weak!(app_weak);
                let render_parse_launch = current_graphtab(&app)
                    .player()
                    .pipeline_description_from_graphview(&current_graphtab(&app).graphview());
                if current_graphtab(&app)
                    .player()
                    .create_pipeline(&render_parse_launch)
                    .is_ok()
                {
                    GPSUI::message::display_message_dialog(
                        &render_parse_launch,
                        gtk::MessageType::Info,
                        |_| {},
                    );
                } else {
                    GPSUI::message::display_error_dialog(
                        false,
                        &format!("Unable to render:\n\n{render_parse_launch}"),
                    );
                }
            });
            let app_weak = app.downgrade();
            app.connect_app_menu_action("graph.pipeline_details", move |_, _| {
                let app = upgrade_weak!(app_weak);
                GPSUI::properties::display_pipeline_details(&app);
            });
            let app_weak = app.downgrade();
            app.connect_app_menu_action("graph.arrange_pipeline", move |_, _| {
                let app = upgrade_weak!(app_weak);
                current_graphtab(&app).graphview().auto_arrange_graph(None);
            });
            app.show_context_menu_at_position(
                &*current_graphtab(&app).graphview(),
                point.to_vec2().x() as f64,
                point.to_vec2().y() as f64,
                &menu,
            );
            None
        }),
    );

    // When user clicks on port with right button
    let app_weak = app.downgrade();
    gt.graphview()
        .connect_local("port-right-clicked", false, move |values: &[Value]| {
            let app = upgrade_weak!(app_weak, None);
            let port_id = values[1].get::<u32>().expect("port id args[1]");
            let node_id = values[2].get::<u32>().expect("node id args[2]");
            let point = values[3]
                .get::<graphene::Point>()
                .expect("point in args[3]");
            let menu: gio::MenuModel = app
                .builder
                .object("port_menu")
                .expect("Couldn't get menu model for port");

            if current_graphtab(&app)
                .graphview()
                .can_remove_port(node_id, port_id)
            {
                let app_weak = app.downgrade();
                app.connect_app_menu_action("port.delete", move |_, _| {
                    let app = upgrade_weak!(app_weak);
                    GPS_DEBUG!("port.delete-link port id {} node id {}", port_id, node_id);
                    current_graphtab(&app)
                        .graphview()
                        .remove_port(node_id, port_id);
                });
            } else {
                app.disconnect_app_menu_action("port.delete");
            }

            let app_weak = app.downgrade();
            app.connect_app_menu_action("port.properties", move |_, _| {
                let app = upgrade_weak!(app_weak);
                GPS_DEBUG!("port.properties port id {} node id {}", port_id, node_id);
                let node = app.node(node_id);
                let port = app.port(node_id, port_id);
                GPSUI::properties::display_pad_properties(
                    &app,
                    &node.name(),
                    &port.name(),
                    node_id,
                    port_id,
                );
            });
            app.show_context_menu_at_position(
                &*current_graphtab(&app).graphview(),
                point.to_vec2().x() as f64,
                point.to_vec2().y() as f64,
                &menu,
            );
            None
        });

    // When user clicks on link with right button
    let app_weak = app.downgrade();
    gt.graphview()
        .connect_local("link-right-clicked", false, move |values: &[Value]| {
            let app = upgrade_weak!(app_weak, None);
            let link_id = values[1].get::<u32>().ok()?;
            let point = values[2].get::<graphene::Point>().ok()?;
            let menu: gio::MenuModel = app.builder.object("link_menu")?;

            let app_weak = app.downgrade();
            app.connect_app_menu_action("link.delete", move |_, _| {
                let app = upgrade_weak!(app_weak);
                GPS_DEBUG!("link.delete id: {}", link_id);
                current_graphtab(&app).graphview().remove_link(link_id);
            });

            app.show_context_menu_at_position(
                &*current_graphtab(&app).graphview(),
                point.to_vec2().x() as f64,
                point.to_vec2().y() as f64,
                &menu,
            );
            None
        });

    // When user clicks on node with right button
    let app_weak = app.downgrade();
    gt.graphview().connect_local(
        "node-right-clicked",
        false,
        glib::clone!(move |values: &[Value]| {
            let app = upgrade_weak!(app_weak, None);
            let node_id = values[1].get::<u32>().expect("node id args[1]");
            let node = current_graphtab(&app).graphview().node(node_id).unwrap();
            let element_exists = GPS::ElementInfo::element_factory_exists(&node.name());
            let point = values[2]
                .get::<graphene::Point>()
                .expect("point in args[2]");
            let menu: gio::MenuModel = app
                .builder
                .object("node_menu")
                .expect("Couldn't get menu model for node");

            let app_weak = app.downgrade();
            app.connect_app_menu_action("node.delete", move |_, _| {
                let app = upgrade_weak!(app_weak);
                GPS_DEBUG!("node.delete id: {}", node_id);
                current_graphtab(&app).graphview().remove_node(node_id);
            });
            if element_exists {
                let app_weak = app.downgrade();
                app.connect_app_menu_action("node.add-to-favorite", move |_, _| {
                    let app = upgrade_weak!(app_weak);
                    GPS_DEBUG!("node.add-to-favorite id: {}", node_id);
                    if let Some(node) = current_graphtab(&app).graphview().node(node_id) {
                        GPSUI::elements::add_to_favorite_list(&app, node.name());
                    };
                });

                let node = app.node(node_id);
                if let Some(input) = GPS::ElementInfo::element_supports_new_pad_request(
                    &node.name(),
                    GM::PortDirection::Input,
                ) {
                    let app_weak = app.downgrade();
                    app.connect_app_menu_action("node.request-pad-input", move |_, _| {
                        let app = upgrade_weak!(app_weak);
                        GPS_DEBUG!("node.request-pad-input id: {}", node_id);
                        app.create_port_with_caps(
                            node_id,
                            GM::PortDirection::Input,
                            GM::PortPresence::Sometimes,
                            input.caps().unwrap_or("ANY").to_string(),
                        );
                    });
                } else {
                    app.disconnect_app_menu_action("node.request-pad-input");
                }
                let node = app.node(node_id);
                if let Some(output) = GPS::ElementInfo::element_supports_new_pad_request(
                    &node.name(),
                    GM::PortDirection::Output,
                ) {
                    let app_weak = app.downgrade();
                    app.connect_app_menu_action("node.request-pad-output", move |_, _| {
                        let app = upgrade_weak!(app_weak);
                        GPS_DEBUG!("node.request-pad-output id: {}", node_id);
                        app.create_port_with_caps(
                            node_id,
                            GM::PortDirection::Output,
                            GM::PortPresence::Sometimes,
                            output.caps().unwrap_or("ANY").to_string(),
                        );
                    });
                } else {
                    app.disconnect_app_menu_action("node.request-pad-output");
                }

                let app_weak = app.downgrade();
                app.connect_app_menu_action("node.properties", move |_, _| {
                    let app = upgrade_weak!(app_weak);
                    GPS_DEBUG!("node.properties id {}", node_id);
                    let node = current_graphtab(&app).graphview().node(node_id).unwrap();
                    GPSUI::properties::display_plugin_properties(&app, &node.name(), node_id);
                });
                let app_weak = app.downgrade();
                app.connect_app_menu_action("node.duplicate", move |_, _| {
                    let app = upgrade_weak!(app_weak);
                    GPS_DEBUG!("node.d id: {}", node_id);
                    if let Some(node) = current_graphtab(&app).graphview().node(node_id) {
                        app.add_new_element(&node.name());
                    };
                });
            }
            app.show_context_menu_at_position(
                &*current_graphtab(&app).graphview(),
                point.to_vec2().x() as f64,
                point.to_vec2().y() as f64,
                &menu,
            );
            None
        }),
    );

    let app_weak = app.downgrade();
    gt.graphview().connect_local(
        "node-double-clicked",
        false,
        glib::clone!(move |values: &[Value]| {
            let app = upgrade_weak!(app_weak, None);
            let node_id = values[1].get::<u32>().expect("node id args[1]");
            GPS_TRACE!("Node double clicked id={}", node_id);
            let node = current_graphtab(&app).graphview().node(node_id).unwrap();
            if GPS::ElementInfo::element_factory_exists(&node.name()) {
                GPSUI::properties::display_plugin_properties(&app, &node.name(), node_id);
            }
            None
        }),
    );
    let app_weak = app.downgrade();
    gt.graphview().connect_local(
        "link-double-clicked",
        false,
        glib::clone!(move |values: &[Value]| {
            let app = upgrade_weak!(app_weak, None);
            let link_id = values[1].get::<u32>().expect("link id args[1]");
            GPS_TRACE!("link double clicked id={}", link_id);
            let link = current_graphtab(&app).graphview().link(link_id).unwrap();
            GPSUI::dialog::get_input(
                &app,
                "Enter caps filter description",
                "description",
                &link.name(),
                move |app, link_desc| {
                    current_graphtab(&app)
                        .graphview()
                        .set_link_name(link.id(), link_desc.as_str());
                    GPS_DEBUG!("link double clicked id={} name={}", link.id(), link.name());
                },
            );
            None
        }),
    );
    let app_weak = app.downgrade();
    gt.graphview().connect_local(
        "link-added",
        false,
        glib::clone!(move |values: &[Value]| {
            let app = upgrade_weak!(app_weak, None);
            let link_id = values[2].get::<u32>().expect("link id args[1]");
            GPS_TRACE!("link added id={}", link_id);
            let graphtab = current_graphtab(&app);
            let graphview = graphtab.graphview();
            let link = graphview.link(link_id).unwrap();
            let port_from = app.port(link.node_from, link.port_from);
            let port_to = app.port(link.node_to, link.port_to);

            // Check caps compatibility if both ports have caps defined.
            // This catches manual port-to-port links that bypass handle_auto_connect().
            if let (Some(caps1), Some(caps2)) = (
                PropertyExt::property(&port_from, "_caps"),
                PropertyExt::property(&port_to, "_caps"),
            ) {
                if !GPS::PadInfo::caps_compatible(&caps1, &caps2) {
                    GPS_WARN!("caps are not compatible caps1={} caps2={}", caps1, caps2);
                    graphview.remove_link(link_id);
                }
            }
            None
        }),
    );
}

impl GPSApp {
    pub fn clear_graph(&self) {
        current_graphtab(self).graphview().clear();
    }

    pub fn save_graph(&self, filename: &str) -> anyhow::Result<()> {
        let mut file = File::create(filename)?;
        let buffer = current_graphtab(self).graphview().render_xml()?;
        file.write_all(&buffer)?;

        Ok(())
    }

    pub fn load_graph(&self, filename: &str, untitled: bool) -> anyhow::Result<()> {
        let mut file = File::open(filename)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| anyhow::anyhow!("Failed to read file {}: {}", filename, e))?;
        let graphtab = current_graphtab(self);
        let graphview = graphtab.graphview();

        // Detect file type by extension (case-insensitive) and load accordingly
        if filename.to_lowercase().ends_with(".dot") {
            // Load DOT file (GStreamer pipeline debug dump)
            let content = String::from_utf8(buffer)
                .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in DOT file: {}", e))?;
            self.load_from_dot(&graphview, &content)?;

            // Auto-arrange since DOT files don't contain position information
            graphview.auto_arrange_graph(None);
            GPS_DEBUG!("Loaded DOT file: {}", filename);
        } else {
            // Load GPS XML file
            graphview.load_from_xml(buffer)?;

            // Restore static pads for nodes that have no ports (with undo disabled)
            graphview.set_undo_recording(false);
            self.restore_static_pads();
            graphview.clear_undo_history();
            graphview.set_undo_recording(true);
        }

        if !untitled {
            current_graphtab_set_filename(self, filename);
        }
        Ok(())
    }

    /// Load a graph from DOT format string (GStreamer pipeline debug dumps)
    ///
    /// Uses GraphView's generic load_from_dot with GstDotLoader to provide
    /// GStreamer-specific element type detection and pad information.
    fn load_from_dot(&self, graphview: &GM::GraphView, content: &str) -> anyhow::Result<()> {
        graphview.load_from_dot(content, &GPS::GstDotLoader)
    }

    pub fn restore_static_pads(&self) {
        let graphtab = current_graphtab(self);
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
                        input.caps().unwrap_or("ANY").to_string(),
                    );
                }

                // Add output pads
                for output in outputs {
                    self.create_port_with_caps(
                        node_id,
                        GM::PortDirection::Output,
                        GM::PortPresence::Always,
                        output.caps().unwrap_or("ANY").to_string(),
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

    /// Attempts to auto-connect a source port to a compatible port on a target node.
    ///
    /// This method implements the auto-connect logic:
    /// 1. First tries to find an existing free port with compatible caps
    /// 2. If there are free ports but none are compatible, fails without creating new ports
    /// 3. Only if all existing ports are linked, tries to create a request pad if supported
    ///
    /// # Arguments
    /// * `graphview` - The graphview where the connection should be made (must be the emitting view)
    /// * `from_node_id` - The ID of the node containing the source port
    /// * `from_port_id` - The ID of the source port
    /// * `target_node_id` - The ID of the target node to connect to
    ///
    /// # Returns
    /// * `Some((node_id, port_id))` if a new request port was created (may need cleanup if link fails)
    /// * `None` if connection was made to an existing port or connection failed
    ///
    /// # Note
    /// The `link-added` signal handler also performs caps validation as defense-in-depth,
    /// but this method validates caps synchronously before creating links to provide
    /// immediate feedback and avoid creating links that would be rejected.
    pub fn handle_auto_connect(
        &self,
        graphview: &GM::GraphView,
        from_node_id: u32,
        from_port_id: u32,
        target_node_id: u32,
    ) -> Option<(u32, u32)> {
        // Validate that source node and port exist in this graphview
        let from_node = match graphview.node(from_node_id) {
            Some(node) => node,
            None => {
                GPS_WARN!(
                    "Auto-connect: source node {} not found in graphview",
                    from_node_id
                );
                return None;
            }
        };

        let from_port = match from_node.port(from_port_id) {
            Some(port) => port,
            None => {
                GPS_WARN!(
                    "Auto-connect: source port {} not found on node {}",
                    from_port_id,
                    from_node_id
                );
                return None;
            }
        };

        let from_caps = PropertyExt::property(&from_port, "_caps").unwrap_or_else(|| {
            GPS_DEBUG!(
                "Auto-connect: port {} has no caps metadata, using ANY (may cause runtime errors)",
                from_port_id
            );
            "ANY".to_string()
        });

        // Determine required direction (opposite of from_port)
        let required_direction = match from_port.direction() {
            GM::PortDirection::Input => GM::PortDirection::Output,
            GM::PortDirection::Output => GM::PortDirection::Input,
            _ => {
                GPS_WARN!("Auto-connect: source port has unknown direction");
                return None;
            }
        };

        // Validate that target node exists in this graphview
        let target_node = match graphview.node(target_node_id) {
            Some(node) => node,
            None => {
                GPS_WARN!(
                    "Auto-connect: target node {} not found in graphview",
                    target_node_id
                );
                return None;
            }
        };

        // 1. Try to find first free compatible existing port
        // Also track if there are any free ports (even if incompatible)
        let mut has_free_port = false;
        for port in target_node.all_ports(required_direction) {
            if graphview.port_is_linked(port.id()).is_none() {
                has_free_port = true;
                let port_caps =
                    PropertyExt::property(&port, "_caps").unwrap_or_else(|| "ANY".to_string());
                if GPS::PadInfo::caps_compatible(&from_caps, &port_caps) {
                    create_directional_link(
                        self,
                        from_node_id,
                        from_port_id,
                        from_port.direction(),
                        target_node_id,
                        port.id(),
                    );
                    GPS_DEBUG!(
                        "Auto-connect: linked port {} to existing port {} on '{}'",
                        from_port_id,
                        port.id(),
                        target_node.name()
                    );
                    return None;
                }
            }
        }

        // 2. Try to create a request pad if element supports it
        // Only create request pads if ALL existing ports are already linked.
        // If there are free ports with incompatible caps, don't create new ports.
        if has_free_port {
            GPS_WARN!(
                "Auto-connect failed: '{}' has free ports but none are compatible with caps '{}'",
                target_node.name(),
                from_caps
            );
            return None;
        }

        if let Some(pad_info) = GPS::ElementInfo::element_supports_new_pad_request(
            &target_node.name(),
            required_direction,
        ) {
            let pad_caps = pad_info.caps().unwrap_or("ANY");
            if GPS::PadInfo::caps_compatible(&from_caps, pad_caps) {
                let new_port_id = self.create_port_with_caps(
                    target_node_id,
                    required_direction,
                    GM::PortPresence::Sometimes,
                    pad_caps.to_string(),
                );

                create_directional_link(
                    self,
                    from_node_id,
                    from_port_id,
                    from_port.direction(),
                    target_node_id,
                    new_port_id,
                );
                GPS_DEBUG!(
                    "Auto-connect: created request port {} on '{}' and linked",
                    new_port_id,
                    target_node.name()
                );
                // Return the new port so it can be cleaned up if the link is rejected
                return Some((target_node_id, new_port_id));
            }
            GPS_WARN!(
                "Auto-connect failed: caps incompatible between '{}' and request pad on '{}'",
                from_caps,
                target_node.name()
            );
        } else {
            GPS_WARN!(
                "Auto-connect failed: no compatible port on '{}' (caps: {})",
                target_node.name(),
                from_caps
            );
        }

        None
    }

    pub fn load_pipeline(&self, pipeline_desc: &str) -> anyhow::Result<()> {
        let graphtab = current_graphtab(self);
        let pd_parsed = pipeline_desc.replace('\\', "");
        graphtab
            .player()
            .graphview_from_pipeline_description(&graphtab.graphview(), &pd_parsed);
        Ok(())
    }
}
