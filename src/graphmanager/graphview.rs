// graphview.rs
//
// Copyright 2021 Tom A. Wagner <tom.a.wagner@protonmail.com>
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GraphManager
//
// SPDX-License-Identifier: GPL-3.0-only

extern crate xml;
use xml::reader::EventReader;
use xml::reader::XmlEvent as XMLREvent;
use xml::writer::EmitterConfig;
use xml::writer::XmlEvent as XMLWEvent;

use super::{
    link::*,
    node::{Node, NodeType},
    port::{Port, PortDirection, PortPresence},
    property::PropertyExt,
    selection::SelectionExt,
};

use once_cell::sync::Lazy;
use std::io::Cursor;

use gtk::{
    gdk,
    glib::{self, clone, subclass::Signal},
    graphene, gsk,
    prelude::*,
    subclass::prelude::*,
};
use log::{debug, error, info, trace, warn};

use std::cell::RefMut;
use std::{cmp::Ordering, collections::HashMap};

static GRAPHVIEW_STYLE: &str = include_str!("graphview.css");
pub static GRAPHVIEW_XML_VERSION: &str = "0.1";

const CANVAS_SIZE: f64 = 5000.0;

/// Context for DOT file loading operations.
///
/// Holds shared state (ID mappings) used across the node, port, and link
/// creation phases when loading a DOT file.
struct DotLoadContext {
    /// Maps DOT cluster IDs to internal node IDs
    node_id_map: HashMap<String, u32>,
    /// Maps DOT port IDs to internal port IDs
    port_id_map: HashMap<String, u32>,
    /// Maps DOT port IDs to their parent node IDs
    node_for_port: HashMap<String, u32>,
    /// Maps normalized instance names to (link_index, is_source_port) pairs
    /// for O(1) lookup during port creation
    links_by_instance: HashMap<String, Vec<(usize, bool)>>,
}

impl DotLoadContext {
    /// Create a new context with pre-built link lookup maps.
    fn new<L: super::dot_parser::DotLoader>(
        dot_graph: &super::dot_parser::DotGraph,
        loader: &L,
    ) -> Self {
        let mut links_by_instance: HashMap<String, Vec<(usize, bool)>> = HashMap::new();

        // Pre-build link lookup maps for O(1) access instead of O(n) iteration
        for (idx, link) in dot_graph.links.iter().enumerate() {
            // Extract instance names from port IDs
            if let Some(instance) = loader.extract_node_instance_from_id(&link.from_port_id) {
                links_by_instance
                    .entry(instance)
                    .or_default()
                    .push((idx, true)); // true = source port
            }
            if let Some(instance) = loader.extract_node_instance_from_id(&link.to_port_id) {
                links_by_instance
                    .entry(instance)
                    .or_default()
                    .push((idx, false)); // false = sink port
            }
        }

        Self {
            node_id_map: HashMap::new(),
            port_id_map: HashMap::new(),
            node_for_port: HashMap::new(),
            links_by_instance,
        }
    }
}

// Default link colors (RGB values 0.0-1.0)
const LINK_COLOR_DEFAULT: (f64, f64, f64) = (0.5, 0.5, 0.5); // Gray
const LINK_COLOR_SELECTED: (f64, f64, f64) = (1.0, 0.18, 0.18); // Red

/// Connection info for edge maps.
///
/// Used to track connections between nodes with port-level detail
/// for improved edge crossing minimization during auto-arrange.
#[derive(Debug, Clone, Copy)]
struct EdgeInfo {
    /// ID of the connected node
    node_id: u32,
    /// Index of the port on that node (used to adjust Y position for better routing)
    port_index: usize,
}

/// Configuration options for auto-arrange
#[derive(Debug, Clone)]
pub struct AutoArrangeOptions {
    /// Horizontal gap between stages (default: 100.0).
    ///
    /// This is the space between the right edge of the widest node in one stage
    /// and the left edge of nodes in the next stage. The actual X distance between
    /// stages depends on node widths plus this gap value.
    pub horizontal_spacing: f32,
    /// Vertical spacing between nodes in the same stage (default: 100.0)
    pub vertical_spacing: f32,
    /// Starting X position for the first stage (default: 50.0)
    pub start_x: f32,
    /// Starting Y position (default: 50.0)
    pub start_y: f32,
    /// Factor for port index offset as fraction of vertical_spacing (default: 0.3).
    /// Higher values spread connections to different ports further apart vertically.
    pub port_offset_factor: f32,
    /// Number of barycenter refinement iterations (default: 4).
    /// More iterations may improve layout quality but increase computation time.
    pub barycenter_iterations: usize,
}

impl Default for AutoArrangeOptions {
    fn default() -> Self {
        Self {
            horizontal_spacing: 100.0,
            vertical_spacing: 100.0,
            start_x: 50.0,
            start_y: 50.0,
            port_offset_factor: 0.3,
            barycenter_iterations: 4,
        }
    }
}

mod imp {
    use super::*;

    use std::cell::{Cell, RefCell};

    use log::warn;

    pub struct DragState {
        node: glib::WeakRef<Node>,
        /// This stores the offset of the pointer to the origin of the node,
        /// so that we can keep the pointer over the same position when moving the node
        ///
        /// The offset is normalized to the default zoom-level of 1.0.
        offset: graphene::Point,
        /// Original position when drag started (for undo)
        original_position: graphene::Point,
    }

    pub struct GraphView {
        pub(super) id: Cell<u32>,
        pub(super) nodes: RefCell<HashMap<u32, (Node, graphene::Point)>>,
        pub(super) links: RefCell<HashMap<u32, Link>>,
        pub(super) current_node_id: Cell<u32>,
        pub(super) current_port_id: Cell<u32>,
        pub(super) current_link_id: Cell<u32>,
        pub(super) port_selected: RefCell<Option<Port>>,
        pub(super) mouse_position: Cell<(f64, f64)>,
        pub dragged_node: RefCell<Option<DragState>>,
        pub hadjustment: RefCell<Option<gtk::Adjustment>>,
        pub vadjustment: RefCell<Option<gtk::Adjustment>>,
        pub zoom_factor: Cell<f64>,
        /// RGB color for links (0.0-1.0 range)
        pub(super) link_color: Cell<(f64, f64, f64)>,
        /// Custom CSS provider for app-injected styles
        pub(super) custom_css_provider: RefCell<Option<gtk::CssProvider>>,
        /// Undo/redo stack for graph operations
        pub(super) undo_stack: RefCell<super::super::undo::UndoStack>,
    }

    impl Default for GraphView {
        fn default() -> Self {
            Self {
                id: Cell::new(0),
                nodes: RefCell::new(HashMap::new()),
                links: RefCell::new(HashMap::new()),
                current_node_id: Cell::new(0),
                current_port_id: Cell::new(0),
                current_link_id: Cell::new(0),
                port_selected: RefCell::new(None),
                mouse_position: Cell::new((0.0, 0.0)),
                dragged_node: RefCell::new(None),
                hadjustment: RefCell::new(None),
                vadjustment: RefCell::new(None),
                zoom_factor: Cell::new(1.0),
                link_color: Cell::new(LINK_COLOR_DEFAULT),
                custom_css_provider: RefCell::new(None),
                undo_stack: RefCell::new(crate::graphmanager::undo::UndoStack::new()),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GraphView {
        const NAME: &'static str = "GraphView";
        type Type = super::GraphView;
        type ParentType = gtk::Widget;
        type Interfaces = (gtk::Scrollable,);

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("graphview");
        }
    }

    impl ObjectImpl for GraphView {
        fn constructed(&self) {
            let obj = self.obj();
            self.parent_constructed();

            self.obj().set_overflow(gtk::Overflow::Hidden);

            let drag_controller = gtk::GestureDrag::new();

            drag_controller.connect_drag_begin(|drag_controller, x, y| {
                let widget = drag_controller
                    .widget()
                    .unwrap()
                    .dynamic_cast::<super::GraphView>()
                    .expect("drag-begin event is not on the GraphView");
                let mut dragged_node = widget.imp().dragged_node.borrow_mut();

                // pick() should at least return the widget itself.
                let target = widget
                    .pick(x, y, gtk::PickFlags::DEFAULT)
                    .expect("drag-begin pick() did not return a widget");
                *dragged_node = if target.ancestor(Port::static_type()).is_some() {
                    // The user targeted a port, so the dragging should be handled by the Port
                    // component instead of here.
                    None
                } else if let Some(target) = target.ancestor(Node::static_type()) {
                    // The user targeted a Node without targeting a specific Port.
                    // Drag the Node around the screen.
                    let node = target.dynamic_cast_ref::<Node>().unwrap();

                    let Some(canvas_node_pos) = widget.node_position(node) else {
                        return;
                    };
                    let canvas_cursor_pos = widget
                        .imp()
                        .screen_space_to_canvas_space_transform()
                        .transform_point(&graphene::Point::new(x as f32, y as f32));

                    Some(DragState {
                        node: node.clone().downgrade(),
                        offset: graphene::Point::new(
                            canvas_cursor_pos.x() - canvas_node_pos.x(),
                            canvas_cursor_pos.y() - canvas_node_pos.y(),
                        ),
                        original_position: canvas_node_pos,
                    })
                } else {
                    None
                }
            });
            drag_controller.connect_drag_update(|drag_controller, x, y| {
                let widget = drag_controller
                    .widget()
                    .unwrap()
                    .dynamic_cast::<super::GraphView>()
                    .expect("drag-update event is not on the GraphView");
                let dragged_node = widget.imp().dragged_node.borrow();
                let Some(DragState { node, offset, .. }) = dragged_node.as_ref() else {
                    return;
                };
                let Some(node) = node.upgrade() else { return };

                let (start_x, start_y) = drag_controller
                    .start_point()
                    .expect("Drag has no start point");

                let onscreen_node_origin =
                    graphene::Point::new((start_x + x) as f32, (start_y + y) as f32);
                let transform = widget.imp().screen_space_to_canvas_space_transform();
                let canvas_node_origin = transform.transform_point(&onscreen_node_origin);

                widget.move_node(
                    &node,
                    &graphene::Point::new(
                        canvas_node_origin.x() - offset.x(),
                        canvas_node_origin.y() - offset.y(),
                    ),
                );
            });

            drag_controller.connect_drag_end(|drag_controller, _x, _y| {
                let widget = drag_controller
                    .widget()
                    .unwrap()
                    .dynamic_cast::<super::GraphView>()
                    .expect("drag-update event is not on the GraphView");

                // Record undo action for node move
                let dragged_node = widget.imp().dragged_node.borrow();
                if let Some(DragState {
                    node,
                    original_position,
                    ..
                }) = dragged_node.as_ref()
                {
                    if let Some(node) = node.upgrade() {
                        if let Some(new_position) = widget.node_position(&node) {
                            // Only record if the position actually changed
                            if (new_position.x() - original_position.x()).abs() > 0.1
                                || (new_position.y() - original_position.y()).abs() > 0.1
                            {
                                widget.imp().undo_stack.borrow_mut().push(
                                    crate::graphmanager::undo::UndoAction::MoveNode {
                                        node_id: node.id(),
                                        old_position: *original_position,
                                        new_position,
                                    },
                                );
                            }
                        }
                    }
                }

                widget.graph_updated();
            });

            let gesture = gtk::GestureClick::new();
            gesture.set_button(0);
            gesture.connect_pressed(clone!(
                #[weak]
                obj,
                #[weak]
                drag_controller,
                move |gesture, _n_press, x, y| {
                    if gesture.current_button() == gdk::BUTTON_SECONDARY {
                        let widget = drag_controller
                            .widget()
                            .unwrap()
                            .dynamic_cast::<Self::Type>()
                            .expect("click event is not on the GraphView");
                        let target = widget
                            .pick(x, y, gtk::PickFlags::DEFAULT)
                            .expect("port pick() did not return a widget");
                        if let Some(target) = target.ancestor(Port::static_type()) {
                            let port = target
                                .dynamic_cast::<Port>()
                                .expect("click event is not on the Port");
                            let node = port
                                .ancestor(Node::static_type())
                                .expect("Unable to reach parent")
                                .dynamic_cast::<Node>()
                                .expect("Unable to cast to Node");
                            obj.emit_by_name::<()>(
                                "port-right-clicked",
                                &[
                                    &port.id(),
                                    &node.id(),
                                    &graphene::Point::new(x as f32, y as f32),
                                ],
                            );
                        } else if let Some(link) = widget.point_on_link(&graphene::Point::new(
                            x.floor() as f32,
                            y.floor() as f32,
                        )) {
                            obj.emit_by_name::<()>(
                                "link-right-clicked",
                                &[&link.id, &graphene::Point::new(x as f32, y as f32)],
                            );
                        } else if let Some(target) = target.ancestor(Node::static_type()) {
                            let node = target
                                .dynamic_cast::<Node>()
                                .expect("click event is not on the Node");
                            widget.unselect_all();
                            node.set_selected(true);
                            obj.emit_by_name::<()>(
                                "node-right-clicked",
                                &[&node.id(), &graphene::Point::new(x as f32, y as f32)],
                            );
                        } else {
                            widget.unselect_all();
                            obj.emit_by_name::<()>(
                                "graph-right-clicked",
                                &[&graphene::Point::new(x as f32, y as f32)],
                            );
                        }
                    } else if gesture.current_button() == gdk::BUTTON_PRIMARY {
                        let widget = drag_controller
                            .widget()
                            .unwrap()
                            .dynamic_cast::<Self::Type>()
                            .expect("click event is not on the GraphView");
                        let target = widget
                            .pick(x, y, gtk::PickFlags::DEFAULT)
                            .expect("port pick() did not return a widget");
                        if let Some(target) = target.ancestor(Port::static_type()) {
                            let port = target
                                .dynamic_cast::<Port>()
                                .expect("click event is not on the Node");
                            widget.unselect_all();
                            port.toggle_selected();
                        } else if let Some(target) = target.ancestor(Node::static_type()) {
                            let node = target
                                .dynamic_cast::<Node>()
                                .expect("click event is not on the Node");
                            widget.unselect_all();
                            node.toggle_selected();
                        } else {
                            widget.point_on_link(&graphene::Point::new(
                                x.floor() as f32,
                                y.floor() as f32,
                            ));
                        }
                    }
                }
            ));

            gesture.connect_released(clone!(
                #[weak]
                gesture,
                #[weak]
                obj,
                #[weak]
                drag_controller,
                move |_gesture, _n_press, x, y| {
                    if gesture.current_button() == gdk::BUTTON_PRIMARY {
                        let widget = drag_controller
                            .widget()
                            .unwrap()
                            .dynamic_cast::<Self::Type>()
                            .expect("click event is not on the GraphView");
                        if let Some(target) = widget.pick(x, y, gtk::PickFlags::DEFAULT) {
                            if let Some(target) = target.ancestor(Port::static_type()) {
                                let port_clicked = target
                                    .dynamic_cast::<Port>()
                                    .expect("click event is not on the Port");
                                if widget.port_is_linked(port_clicked.id()).is_none() {
                                    let selected_port = widget.selected_port().to_owned();
                                    if let Some(mut port_from) = selected_port {
                                        debug!(
                                            "Port {} is clicked at {}:{}",
                                            port_clicked.id(),
                                            x,
                                            y
                                        );
                                        let mut port_to = port_clicked;
                                        if widget.ports_compatible(&port_to) {
                                            let mut node_from = port_from
                                                .ancestor(Node::static_type())
                                                .expect("Unable to reach parent")
                                                .dynamic_cast::<Node>()
                                                .expect("Unable to cast to Node");
                                            let mut node_to = port_to
                                                .ancestor(Node::static_type())
                                                .expect("Unable to reach parent")
                                                .dynamic_cast::<Node>()
                                                .expect("Unable to cast to Node");
                                            info!(
                                                "add link from port {} to {} ",
                                                port_from.id(),
                                                port_to.id()
                                            );
                                            if port_to.direction() == PortDirection::Output {
                                                debug!("swap ports and nodes to create the link");
                                                std::mem::swap(&mut node_from, &mut node_to);
                                                std::mem::swap(&mut port_from, &mut port_to);
                                            }
                                            widget.add_link(widget.create_link(
                                                node_from.id(),
                                                node_to.id(),
                                                port_from.id(),
                                                port_to.id(),
                                            ));
                                        }
                                        widget.set_selected_port(None);
                                    } else {
                                        info!("add selected port id {}", port_clicked.id());
                                        widget.set_selected_port(Some(&port_clicked));
                                    }
                                } else {
                                    // click to a linked port
                                    widget.set_selected_port(None);
                                }
                            } else if let Some(target) = target.ancestor(Node::static_type()) {
                                let node = target
                                    .dynamic_cast::<Node>()
                                    .expect("click event is not on the Node");

                                // Check if we have a selected port for auto-connect
                                // Clone the port first to avoid borrow conflict with set_selected_port
                                let selected_port_clone = widget.selected_port().clone();
                                if let Some(from_port) = selected_port_clone {
                                    // Get parent node with graceful error handling
                                    let from_node_opt = from_port
                                        .ancestor(Node::static_type())
                                        .and_then(|ancestor| ancestor.dynamic_cast::<Node>().ok());

                                    if let Some(from_node) = from_node_opt {
                                        // Only auto-connect if clicking on a different node
                                        if from_node.id() != node.id() {
                                            // Find a free port with opposite direction on target node
                                            let required_direction = match from_port.direction() {
                                                PortDirection::Input => PortDirection::Output,
                                                PortDirection::Output => PortDirection::Input,
                                                _ => PortDirection::Unknown,
                                            };

                                            if required_direction != PortDirection::Unknown {
                                                // Emit signal for app layer to handle node link request
                                                obj.emit_by_name::<()>(
                                                    "node-link-request",
                                                    &[&from_node.id(), &from_port.id(), &node.id()],
                                                );
                                            }
                                        }
                                    } else {
                                        warn!("Port has no valid parent node, cannot auto-connect");
                                    }
                                    widget.set_selected_port(None);
                                } else {
                                    info!(" node id {}", node.id());
                                    if _n_press % 2 == 0 {
                                        info!("double clicked node id {}", node.id());
                                        obj.emit_by_name::<()>(
                                            "node-double-clicked",
                                            &[
                                                &node.id(),
                                                &graphene::Point::new(x as f32, y as f32),
                                            ],
                                        );
                                    }
                                }
                            } else if _n_press % 2 == 0 {
                                if let Some(link) = widget.point_on_link(&graphene::Point::new(
                                    x.floor() as f32,
                                    y.floor() as f32,
                                )) {
                                    info!("double clicked link id {}", link.id());
                                    obj.emit_by_name::<()>(
                                        "link-double-clicked",
                                        &[&link.id(), &graphene::Point::new(x as f32, y as f32)],
                                    );
                                }
                                // Click to something else than a port or node
                                widget.set_selected_port(None);
                            } else {
                                info!("click {}", widget.width());
                                // Click to something else than a port or node
                                widget.set_selected_port(None);
                            }
                        }
                    }
                }
            ));
            obj.add_controller(drag_controller);
            obj.add_controller(gesture);

            let event_motion = gtk::EventControllerMotion::new();
            event_motion.connect_motion(glib::clone!(
                #[weak]
                obj,
                move |_e, x, y| {
                    let graphview = obj;
                    if graphview.selected_port().is_some() {
                        graphview.set_mouse_position(x, y);
                        graphview.queue_allocate();
                    }
                }
            ));
            obj.add_controller(event_motion);

            let scroll_controller =
                gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);

            scroll_controller.connect_scroll(|eventcontroller, _, delta_y| {
                let event = eventcontroller.current_event().unwrap(); // We are inside the event handler, so it must have an event

                if event
                    .modifier_state()
                    .contains(gdk::ModifierType::CONTROL_MASK)
                {
                    let widget = eventcontroller
                        .widget()
                        .unwrap()
                        .downcast::<super::GraphView>()
                        .unwrap();
                    widget.set_zoom_factor(widget.zoom_factor() + (0.1 * -delta_y), None);

                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
            self.obj().add_controller(scroll_controller);
        }

        fn dispose(&self) {
            // Remove custom CSS provider from display
            if let Some(provider) = self.custom_css_provider.borrow().as_ref() {
                if let Some(display) = gtk::gdk::Display::default() {
                    gtk::style_context_remove_provider_for_display(&display, provider);
                }
            }

            self.nodes
                .borrow()
                .values()
                .for_each(|(node, _)| node.unparent())
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("port-right-clicked")
                        .param_types([
                            u32::static_type(),
                            u32::static_type(),
                            graphene::Point::static_type(),
                        ])
                        .build(),
                    Signal::builder("node-right-clicked")
                        .param_types([u32::static_type(), graphene::Point::static_type()])
                        .build(),
                    Signal::builder("node-double-clicked")
                        .param_types([u32::static_type(), graphene::Point::static_type()])
                        .build(),
                    Signal::builder("link-right-clicked")
                        .param_types([u32::static_type(), graphene::Point::static_type()])
                        .build(),
                    Signal::builder("graph-right-clicked")
                        .param_types([graphene::Point::static_type()])
                        .build(),
                    Signal::builder("graph-updated")
                        .param_types(
                            // returns graph ID
                            [u32::static_type()],
                        )
                        .build(),
                    Signal::builder("node-added")
                        .param_types(
                            // returns graph ID and Node ID
                            [u32::static_type(), u32::static_type()],
                        )
                        .build(),
                    Signal::builder("port-added")
                        .param_types([u32::static_type(), u32::static_type(), u32::static_type()])
                        .build(),
                    Signal::builder("link-added")
                        .param_types([u32::static_type(), u32::static_type()])
                        .build(),
                    Signal::builder("link-removed")
                        .param_types([u32::static_type(), u32::static_type()])
                        .build(),
                    Signal::builder("link-double-clicked")
                        .param_types([u32::static_type(), graphene::Point::static_type()])
                        .build(),
                    Signal::builder("node-link-request")
                        .param_types([u32::static_type(), u32::static_type(), u32::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("hadjustment"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("vadjustment"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("hscroll-policy"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("vscroll-policy"),
                    glib::ParamSpecDouble::builder("zoom-factor")
                        .minimum(0.3)
                        .maximum(4.0)
                        .default_value(1.0)
                        .flags(glib::ParamFlags::CONSTRUCT | glib::ParamFlags::READWRITE)
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "hadjustment" => self.hadjustment.borrow().to_value(),
                "vadjustment" => self.vadjustment.borrow().to_value(),
                "hscroll-policy" | "vscroll-policy" => gtk::ScrollablePolicy::Natural.to_value(),
                "zoom-factor" => self.zoom_factor.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "hadjustment" => {
                    obj.set_adjustment(&obj, value.get().ok(), gtk::Orientation::Horizontal)
                }
                "vadjustment" => {
                    obj.set_adjustment(&obj, value.get().ok(), gtk::Orientation::Vertical)
                }
                "hscroll-policy" | "vscroll-policy" => {}
                "zoom-factor" => {
                    self.zoom_factor.set(value.get().unwrap());
                    obj.queue_allocate();
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for GraphView {
        fn size_allocate(&self, _width: i32, _height: i32, baseline: i32) {
            let widget = &*self.obj();

            for (node, point) in self.nodes.borrow().values() {
                let (_, natural_size) = node.preferred_size();

                let transform = self
                    .canvas_space_to_screen_space_transform()
                    .translate(point);

                node.allocate(
                    natural_size.width(),
                    natural_size.height(),
                    baseline,
                    Some(transform),
                );
            }

            if let Some(ref hadjustment) = *self.hadjustment.borrow() {
                widget.set_adjustment_values(widget, hadjustment, gtk::Orientation::Horizontal);
            }
            if let Some(ref vadjustment) = *self.vadjustment.borrow() {
                widget.set_adjustment_values(widget, vadjustment, gtk::Orientation::Vertical);
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            /* FIXME: A lot of hardcoded values in here.
            Try to use relative units (em) and colours from the theme as much as possible. */
            let widget = &*self.obj();
            let alloc = widget.allocation();
            // Draw all children
            // Draw all visible children
            self.nodes
                .borrow()
                .values()
                // Cull nodes from rendering when they are outside the visible canvas area
                .filter(|(node, _)| alloc.intersect(&node.allocation()).is_some())
                .for_each(|(node, _)| widget.snapshot_child(node, snapshot));

            for link in self.links.borrow().values() {
                if let Some((from_x, from_y, to_x, to_y)) = self.link_coordinates(link) {
                    self.draw_link(
                        snapshot,
                        link.active(),
                        link.selected(),
                        link.name().as_str(),
                        link.thickness as f64,
                        &graphene::Point::new(from_x as f32, from_y as f32),
                        &graphene::Point::new(to_x as f32, to_y as f32),
                    );
                } else {
                    warn!("Could not get link coordinates: {:?}", link);
                }
            }

            if self.port_selected.borrow().is_some() {
                let port = self.port_selected.borrow();
                let port = port.as_ref().unwrap();
                let node = port
                    .ancestor(Node::static_type())
                    .expect("Unable to reach parent")
                    .dynamic_cast::<Node>()
                    .expect("Unable to cast to Node");
                let (from_x, from_y) = self.link_from_coordinates(node.id(), port.id());
                let (to_x, to_y) = self.mouse_position.get();
                self.draw_link(
                    snapshot,
                    false,
                    false,
                    "",
                    2.0,
                    &graphene::Point::new(from_x as f32, from_y as f32),
                    &graphene::Point::new(to_x as f32, to_y as f32),
                );
            }
        }
    }

    impl ScrollableImpl for GraphView {}

    impl GraphView {
        /// Returns a [`gsk::Transform`] matrix that can translate from canvas space to screen space.
        ///
        /// Canvas space is non-zoomed, and (0, 0) is fixed at the middle of the graph. \
        /// Screen space is zoomed and adjusted for scrolling, (0, 0) is at the top-left corner of the window.
        ///
        /// This is the inverted form of [`Self::screen_space_to_canvas_space_transform()`].
        fn canvas_space_to_screen_space_transform(&self) -> gsk::Transform {
            let hadj = self.hadjustment.borrow().as_ref().unwrap().value();
            let vadj = self.vadjustment.borrow().as_ref().unwrap().value();
            let zoom_factor = self.zoom_factor.get();

            gsk::Transform::new()
                .translate(&graphene::Point::new(-hadj as f32, -vadj as f32))
                .scale(zoom_factor as f32, zoom_factor as f32)
        }

        /// Returns a [`gsk::Transform`] matrix that can translate from screen space to canvas space.
        ///
        /// This is the inverted form of [`Self::canvas_space_to_screen_space_transform()`], see that function for a more detailed explanation.
        fn screen_space_to_canvas_space_transform(&self) -> gsk::Transform {
            self.canvas_space_to_screen_space_transform()
                .invert()
                .unwrap()
        }

        fn link_from_coordinates(&self, node_from: u32, port_from: u32) -> (f64, f64) {
            let nodes = self.nodes.borrow();
            let widget = &*self.obj();
            let from_node = nodes
                .get(&node_from)
                .unwrap_or_else(|| panic!("Unable to get node from {}", node_from));

            let from_port = from_node
                .0
                .port(port_from)
                .unwrap_or_else(|| panic!("Unable to get port from {}", port_from));

            // Get the port's center position for link anchor
            let anchor = from_port.get_link_anchor();
            let (x, y) = from_port
                .translate_coordinates(widget, anchor.x() as f64, anchor.y() as f64)
                .unwrap();

            (x, y)
        }

        fn link_to_coordinates(&self, node_to: u32, port_to: u32) -> (f64, f64) {
            let nodes = self.nodes.borrow();
            let widget = &*self.obj();

            let to_node = nodes
                .get(&node_to)
                .unwrap_or_else(|| panic!("Unable to get node to {}", node_to));
            let to_port = to_node
                .0
                .port(port_to)
                .unwrap_or_else(|| panic!("Unable to get port to {}", port_to));

            // Get the port's center position for link anchor
            let anchor = to_port.get_link_anchor();
            let (x, y) = to_port
                .translate_coordinates(widget, anchor.x() as f64, anchor.y() as f64)
                .unwrap();

            (x, y)
        }
        /// Retrieves coordinates for the drawn link to start at and to end at.
        ///
        /// # Returns
        /// `Some((from_x, from_y, to_x, to_y))` if all objects the links refers to exist as widgets.
        pub fn link_coordinates(&self, link: &Link) -> Option<(f64, f64, f64, f64)> {
            let (from_x, from_y) = self.link_from_coordinates(link.node_from, link.port_from);
            let (to_x, to_y) = self.link_to_coordinates(link.node_to, link.port_to);
            Some((from_x, from_y, to_x, to_y))
        }
        #[allow(clippy::too_many_arguments)]
        fn draw_link(
            &self,
            snapshot: &gtk::Snapshot,
            active: bool,
            selected: bool,
            name: &str,
            thickness: f64,
            point_from: &graphene::Point,
            point_to: &graphene::Point,
        ) {
            let alloc = self.obj().allocation();

            let link_cr = snapshot.append_cairo(&graphene::Rect::new(
                0.0,
                0.0,
                alloc.width() as f32,
                alloc.height() as f32,
            ));
            link_cr.set_line_width(thickness);
            // Use dashed line for inactive links, full line otherwise.
            if active {
                link_cr.set_dash(&[], 0.0);
            } else {
                link_cr.set_dash(&[10.0, 5.0], 0.0);
            }

            // Set link color based on selection state or custom color
            let color = if selected {
                LINK_COLOR_SELECTED
            } else {
                self.link_color.get()
            };
            link_cr.set_source_rgb(color.0, color.1, color.2);

            link_cr.move_to(point_from.x() as f64, point_from.y() as f64);
            link_cr.line_to(point_to.x() as f64, point_to.y() as f64);
            link_cr.set_line_width(2.0);

            if let Err(e) = link_cr.stroke() {
                warn!("Failed to draw graphview links: {}", e);
            };
            trace!("the link name is {}", name);
            if !name.is_empty() {
                let x = (point_from.x() + point_to.x()) / 2.0 + 20.0;
                let y = (point_from.y() + point_to.y()) / 2.0 + 20.0;
                link_cr.move_to(x as f64, y as f64);
                let _ = link_cr.show_text(name);
            }
        }
    }
}

glib::wrapper! {
    pub struct GraphView(ObjectSubclass<imp::GraphView>)
        @extends gtk::Widget, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget,
        @implements gtk::Scrollable;
}

impl GraphView {
    pub const ZOOM_MIN: f64 = 0.3;
    pub const ZOOM_MAX: f64 = 4.0;
    /// Create a new graphview
    ///
    /// # Returns
    /// Graphview object
    pub fn new() -> Self {
        // Load CSS from the STYLE variable.
        let provider = gtk::CssProvider::new();
        provider.load_from_data(GRAPHVIEW_STYLE);
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("Error initializing gtk css provider."),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        glib::Object::new::<Self>()
    }

    /// Set graphview id
    ///
    pub fn set_id(&self, id: u32) {
        let private = imp::GraphView::from_obj(self);
        private.id.set(id)
    }

    /// Retrieves the graphview id
    ///
    pub fn id(&self) -> u32 {
        let private = imp::GraphView::from_obj(self);
        private.id.get()
    }

    /// Set dark theme mode
    ///
    /// When enabled, applies the dark-theme CSS class for dark mode styling
    pub fn set_dark_theme(&self, dark: bool) {
        if dark {
            self.add_css_class("dark-theme");
        } else {
            self.remove_css_class("dark-theme");
        }
    }

    /// Check if dark theme is enabled
    pub fn is_dark_theme(&self) -> bool {
        self.has_css_class("dark-theme")
    }

    /// Set the link color for rendering
    ///
    /// Changes the color of all links in the graph view.
    /// Color values should be in the range 0.0-1.0 for RGB.
    pub fn set_link_color(&self, r: f64, g: f64, b: f64) {
        let private = imp::GraphView::from_obj(self);
        let new_color = (r, g, b);
        if private.link_color.get() != new_color {
            private.link_color.set(new_color);
            self.queue_draw();
        }
    }

    /// Get the current link color
    pub fn link_color(&self) -> (f64, f64, f64) {
        let private = imp::GraphView::from_obj(self);
        private.link_color.get()
    }

    /// Set custom CSS for the graphview
    ///
    /// This allows the app to inject custom CSS that overrides the default styles.
    /// The CSS is applied with USER priority, which is higher than the default
    /// APPLICATION priority, allowing it to override built-in styles.
    ///
    /// # Arguments
    /// * `css` - CSS string to apply. Pass an empty string to clear custom CSS.
    pub fn set_custom_css(&self, css: &str) {
        let private = imp::GraphView::from_obj(self);
        let display = gtk::gdk::Display::default().expect("Could not get default display");

        // Remove existing custom provider if any
        if let Some(old_provider) = private.custom_css_provider.borrow().as_ref() {
            gtk::style_context_remove_provider_for_display(&display, old_provider);
        }

        if css.is_empty() {
            *private.custom_css_provider.borrow_mut() = None;
        } else {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(css);
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
            *private.custom_css_provider.borrow_mut() = Some(provider);
        }

        self.queue_draw();
    }

    /// Clear the graphview
    ///
    pub fn clear(&self) {
        self.remove_all_nodes();
        self.graph_updated();
    }

    pub fn zoom_factor(&self) -> f64 {
        self.property("zoom-factor")
    }

    pub fn set_zoom_factor(&self, zoom_factor: f64, anchor: Option<(f64, f64)>) {
        let private = imp::GraphView::from_obj(self);
        let zoom_factor = zoom_factor.clamp(Self::ZOOM_MIN, Self::ZOOM_MAX);

        let (anchor_x_screen, anchor_y_screen) = anchor.unwrap_or_else(|| {
            (
                self.allocation().width() as f64 / 2.0,
                self.allocation().height() as f64 / 2.0,
            )
        });

        let old_zoom = private.zoom_factor.get();
        let hadjustment_ref = private.hadjustment.borrow();
        let vadjustment_ref = private.vadjustment.borrow();
        let hadjustment = hadjustment_ref.as_ref().unwrap();
        let vadjustment = vadjustment_ref.as_ref().unwrap();

        let x_total = (anchor_x_screen + hadjustment.value()) / old_zoom;
        let y_total = (anchor_y_screen + vadjustment.value()) / old_zoom;

        let new_hadjustment = x_total * zoom_factor - anchor_x_screen;
        let new_vadjustment = y_total * zoom_factor - anchor_y_screen;

        hadjustment.set_value(new_hadjustment);
        vadjustment.set_value(new_vadjustment);

        self.set_property("zoom-factor", zoom_factor);
        info!("zoom factor {}", zoom_factor);
    }

    // Node

    /// Create a new node with a new id
    ///
    pub fn create_node(&self, name: &str, node_type: NodeType) -> Node {
        let id = self.next_node_id();
        self.create_node_with_id(id, name, node_type)
    }

    /// Create a new node and add it to the graphview with input/output port number.
    ///
    pub fn create_node_with_port(
        &self,
        name: &str,
        node_type: NodeType,
        input: u32,
        output: u32,
    ) -> Node {
        let mut node = self.create_node(name, node_type);

        let _i = 0;
        for _i in 0..input {
            let port = self.create_port("in", PortDirection::Input, PortPresence::Always);
            self.add_port_to_node(&mut node, port);
        }
        let _i = 0;
        for _i in 0..output {
            let port = self.create_port("out", PortDirection::Output, PortPresence::Always);
            self.add_port_to_node(&mut node, port);
        }
        node
    }

    /// Add node to the graphview without port
    ///
    pub fn add_node(&self, node: Node) {
        let private = imp::GraphView::from_obj(self);
        node.set_parent(self);

        // Place widgets in columns of 3, growing down
        let x = if let Some(node_type) = node.node_type() {
            match node_type {
                NodeType::Source => 20.0,
                NodeType::Transform => 320.0,
                NodeType::Sink => 620.0,
                _ => 20.0,
            }
        } else {
            420.0
        };

        let y = private
            .nodes
            .borrow()
            .values()
            .filter(|n| {
                // Only look at nodes of the same type
                n.0.node_type() == node.node_type()
            })
            .filter_map(|n| {
                // Map nodes to their Y positions
                self.node_position(&n.0.clone().upcast())
                    .map(|point| point.y())
            })
            .max_by(|y1, y2| {
                // Get max Y in column
                y1.partial_cmp(y2).unwrap_or(Ordering::Equal)
            })
            .map_or(20_f32, |y| y + 120.0);

        let node_id = node.id();
        // Update the node's internal position so it gets saved correctly
        node.set_position(x, y);
        let position = graphene::Point::new(x, y);

        // Record undo action
        private
            .undo_stack
            .borrow_mut()
            .push(crate::graphmanager::undo::UndoAction::AddNode {
                node_data: crate::graphmanager::undo::NodeData::from_node(&node),
                position,
            });

        private
            .nodes
            .borrow_mut()
            .insert(node.id(), (node, position));
        self.emit_by_name::<()>("node-added", &[&private.id.get(), &node_id]);
        self.graph_updated();

        // Scroll view to show the newly added node
        self.scroll_to_position(x, y);
    }

    /// Remove node from the graphview
    ///
    pub fn remove_node(&self, id: u32) {
        let private = imp::GraphView::from_obj(self);

        // Collect node data and connected links before removal
        let (node_data, position, connected_links) = {
            let nodes = private.nodes.borrow();
            if let Some(node) = nodes.get(&id) {
                let node_data = crate::graphmanager::undo::NodeData::from_node(&node.0);
                let position = node.1;
                let mut connected_links = Vec::new();

                // Collect all links connected to this node
                for link in private.links.borrow().values() {
                    if link.node_from == id || link.node_to == id {
                        connected_links.push(crate::graphmanager::undo::LinkData {
                            id: link.id,
                            node_from: link.node_from,
                            node_to: link.node_to,
                            port_from: link.port_from,
                            port_to: link.port_to,
                            active: link.active(),
                            name: link.name(),
                        });
                    }
                }

                (Some(node_data), position, connected_links)
            } else {
                (None, graphene::Point::zero(), Vec::new())
            }
        }; // Drop borrow here

        if let Some(node_data) = node_data {
            // Record undo action before removing
            private.undo_stack.borrow_mut().push(
                crate::graphmanager::undo::UndoAction::RemoveNode {
                    node_data,
                    position,
                    connected_links,
                },
            );

            // Now actually remove the node
            if let Some(node) = private.nodes.borrow_mut().remove(&id) {
                while let Some(link_id) = self.node_is_linked(node.0.id()) {
                    info!("Remove link id {}", link_id);
                    private.links.borrow_mut().remove(&link_id);
                }
                node.0.unparent();
            }
        } else {
            warn!("Tried to remove non-existent node (id={}) from graph", id);
        }
    }

    /// Select all nodes according to the NodeType
    ///
    /// Returns a vector of nodes
    pub fn all_nodes(&self, node_type: NodeType) -> Vec<Node> {
        let private = imp::GraphView::from_obj(self);
        let nodes = private.nodes.borrow();
        let nodes_list: Vec<_> = nodes
            .iter()
            .filter(|(_, node)| {
                *node.0.node_type().unwrap() == node_type || node_type == NodeType::All
            })
            .map(|(_, node)| node.0.clone())
            .collect();
        nodes_list
    }

    /// Get the node with the specified node id inside the graphview.
    ///
    /// Returns `None` if the node is not in the graphview.
    pub fn node(&self, id: u32) -> Option<Node> {
        let private = imp::GraphView::from_obj(self);

        if let Some(node) = private.nodes.borrow().get(&id).cloned() {
            Some(node.0)
        } else {
            None
        }
    }

    /// Get the node with the specified node name inside the graphview.
    ///
    /// Returns `None` if the node is not in the graphview.
    pub fn node_by_unique_name(&self, unique_name: &str) -> Option<Node> {
        let private = imp::GraphView::from_obj(self);
        for node in private.nodes.borrow().values() {
            if node.0.unique_name() == unique_name {
                return Some(node.0.clone());
            }
        }
        None
    }

    /// Remove all the nodes from the graphview
    ///
    pub fn remove_all_nodes(&self) {
        let private = imp::GraphView::from_obj(self);
        let nodes_list = self.all_nodes(NodeType::All);
        for node in nodes_list {
            self.remove_node(node.id());
        }
        private.current_node_id.set(0);
        private.current_port_id.set(0);
        private.current_link_id.set(0);
    }

    /// Check if the node is linked
    ///
    /// Returns Some(link id) or `None` if the node is not linked.
    pub fn node_is_linked(&self, node_id: u32) -> Option<u32> {
        let private = imp::GraphView::from_obj(self);
        for (key, link) in private.links.borrow().iter() {
            if link.node_from == node_id || link.node_to == node_id {
                return Some(*key);
            }
        }
        None
    }

    /// Get the position of the specified node inside the graphview.
    ///
    /// Returns `None` if the node is not in the graphview.
    pub(super) fn node_position(&self, node: &Node) -> Option<graphene::Point> {
        self.imp()
            .nodes
            .borrow()
            .get(&node.id())
            .map(|(_, point)| *point)
    }

    // Port

    /// Create a new port with a new id
    ///
    pub fn create_port(
        &self,
        name: &str,
        direction: PortDirection,
        presence: PortPresence,
    ) -> Port {
        let id = self.next_port_id();
        info!("Create a port with port id {}", id);

        self.create_port_with_id(id, name, direction, presence)
    }

    /// Add the port with id from node with id.
    ///
    pub fn add_port_to_node(&self, node: &mut Node, port: Port) {
        let private = imp::GraphView::from_obj(self);
        let port_id = port.id();
        node.add_port(port);

        self.emit_by_name::<()>("port-added", &[&private.id.get(), &node.id(), &port_id]);
    }

    /// Check if the port with id from node with id can be removed.
    ///
    /// Return true if the port presence is not always.
    pub fn can_remove_port(&self, node_id: u32, port_id: u32) -> bool {
        let private = imp::GraphView::from_obj(self);
        if let Some(node) = private.nodes.borrow().get(&node_id) {
            return node.0.can_remove_port(port_id);
        }
        warn!("Unable to find a node with the id {}", node_id);
        false
    }

    /// Remove the port with id from node with id.
    ///
    pub fn remove_port(&self, node_id: u32, port_id: u32) {
        let private = imp::GraphView::from_obj(self);
        if let Some(node) = private.nodes.borrow().get(&node_id) {
            if let Some(link_id) = self.port_is_linked(port_id) {
                self.remove_link(link_id);
            }
            node.0.remove_port(port_id);
        }
    }

    /// Check if the port is linked
    ///
    /// Returns Some(link id) or `None` if the port is not linked.
    pub fn port_is_linked(&self, port_id: u32) -> Option<u32> {
        let private = imp::GraphView::from_obj(self);
        for (key, link) in private.links.borrow().iter() {
            if link.port_from == port_id || link.port_to == port_id {
                return Some(*key);
            }
        }
        None
    }

    // Link

    /// Create a new link with a new id
    ///
    pub fn create_link(
        &self,
        node_from_id: u32,
        node_to_id: u32,
        port_from_id: u32,
        port_to_id: u32,
    ) -> Link {
        self.create_link_with_id(
            self.next_link_id(),
            node_from_id,
            node_to_id,
            port_from_id,
            port_to_id,
        )
    }

    /// Add a link to the graphView
    ///
    pub fn add_link(&self, link: Link) {
        let private = imp::GraphView::from_obj(self);
        if !self.link_exists(&link) {
            let link_id = link.id;

            // Record undo action
            private
                .undo_stack
                .borrow_mut()
                .push(crate::graphmanager::undo::UndoAction::AddLink {
                    link_data: crate::graphmanager::undo::LinkData {
                        id: link.id,
                        node_from: link.node_from,
                        node_to: link.node_to,
                        port_from: link.port_from,
                        port_to: link.port_to,
                        active: link.active(),
                        name: link.name(),
                    },
                });

            private.links.borrow_mut().insert(link_id, link);
            self.emit_by_name::<()>("link-added", &[&private.id.get(), &link_id]);
            self.graph_updated();
        }
    }

    /// Set the link state with ink id and link state (boolean)
    ///
    pub fn set_link_state(&self, link_id: u32, active: bool) {
        let private = imp::GraphView::from_obj(self);
        if let Some(link) = private.links.borrow_mut().get_mut(&link_id) {
            link.set_active(active);
            self.queue_draw();
        } else {
            warn!("Link state changed on unknown link (id={})", link_id);
        }
    }

    /// Select all nodes according to the NodeType
    ///
    /// Returns a vector of links
    pub fn all_links(&self, link_state: bool) -> Vec<Link> {
        let private = imp::GraphView::from_obj(self);
        let links = private.links.borrow();
        let links_list: Vec<_> = links
            .iter()
            .filter(|(_, link)| link.active() == link_state)
            .map(|(_, node)| node.clone())
            .collect();
        links_list
    }

    /// Get the link with the specified link id inside the graphview.
    ///
    /// Returns `None` if the link is not in the graphview.
    pub fn link(&self, id: u32) -> Option<Link> {
        let private = imp::GraphView::from_obj(self);
        private.links.borrow().get(&id).cloned()
    }

    /// Set the link state with ink id and link state (boolean)
    ///
    pub fn set_link_name(&self, link_id: u32, name: &str) {
        let private = imp::GraphView::from_obj(self);
        let mut updated = false;
        if let Some(link) = private.links.borrow_mut().get_mut(&link_id) {
            link.set_name(name);
            self.queue_draw();
            updated = true;
        } else {
            warn!("Link name changed on unknown link (id={})", link_id);
        }
        if updated {
            self.graph_updated();
        }
    }

    /// Retrieves the node/port id connected to the input port id
    ///
    pub fn port_connected_to(&self, port_id: u32) -> Option<(u32, u32)> {
        let private = imp::GraphView::from_obj(self);
        for (_id, link) in private.links.borrow().iter() {
            if port_id == link.port_from {
                return Some((link.port_to, link.node_to));
            }
        }
        None
    }

    /// Retrieves the link connected to the port id
    ///
    pub fn port_link(&self, port_id: u32) -> Option<Link> {
        let private = imp::GraphView::from_obj(self);
        for (_id, link) in private.links.borrow().iter() {
            if port_id == link.port_from {
                return Some(link.clone());
            }
        }
        None
    }

    /// Delete the selected element (link, node, port)
    ///
    pub fn delete_selected(&self) {
        let private = imp::GraphView::from_obj(self);
        let mut link_id = None;
        let mut node_id = None;
        for link in private.links.borrow_mut().values() {
            if link.selected() {
                link_id = Some(link.id);
            }
        }
        for node in private.nodes.borrow_mut().values() {
            if node.0.selected() {
                node_id = Some(node.0.id());
            }
        }
        if let Some(id) = link_id {
            self.remove_link(id);
        }
        if let Some(id) = node_id {
            self.remove_node(id);
        }

        self.graph_updated();
    }

    /// Render the graph with XML format in a buffer
    ///
    pub fn render_xml(&self) -> anyhow::Result<Vec<u8>> {
        let private = imp::GraphView::from_obj(self);

        let mut buffer = Vec::new();
        let mut writer = EmitterConfig::new()
            .perform_indent(true)
            .create_writer(&mut buffer);

        writer.write(
            XMLWEvent::start_element("Graph")
                .attr("id", &private.id.get().to_string())
                .attr("version", GRAPHVIEW_XML_VERSION),
        )?;

        //Get the nodes

        for node in self.all_nodes(NodeType::All) {
            writer.write(
                XMLWEvent::start_element("Node")
                    .attr("name", &node.name())
                    .attr("id", &node.id().to_string())
                    .attr("type", &node.node_type().unwrap().to_string())
                    .attr("pos_x", &node.position().0.to_string())
                    .attr("pos_y", &node.position().1.to_string())
                    .attr("light", &node.light().to_string()),
            )?;
            // Sort ports by name to ensure consistent ordering when saving/loading
            // This preserves visual port positions (e.g., sink_0 before sink_1)
            let mut ports: Vec<_> = node.ports().values().cloned().collect();
            ports.sort_by_key(|p| p.name());
            for port in ports {
                writer.write(
                    XMLWEvent::start_element("Port")
                        .attr("name", &port.name())
                        .attr("id", &port.id().to_string())
                        .attr("direction", &port.direction().to_string())
                        .attr("presence", &port.presence().to_string()),
                )?;
                for (name, value) in port.properties().iter() {
                    writer.write(
                        XMLWEvent::start_element("Property")
                            .attr("name", name)
                            .attr("value", value),
                    )?;
                    writer.write(XMLWEvent::end_element())?;
                }
                writer.write(XMLWEvent::end_element())?;
            }

            for (name, value) in node.properties().iter() {
                info!("  Saving property: {}={}", name, value);
                writer.write(
                    XMLWEvent::start_element("Property")
                        .attr("name", name)
                        .attr("value", value),
                )?;
                writer.write(XMLWEvent::end_element())?;
            }
            writer.write(XMLWEvent::end_element())?;
        }
        //Get the link and write it.
        for (_id, link) in private.links.borrow().iter() {
            writer.write(
                XMLWEvent::start_element("Link")
                    .attr("id", &link.id.to_string())
                    .attr("node_from", &link.node_from.to_string())
                    .attr("node_to", &link.node_to.to_string())
                    .attr("port_from", &link.port_from.to_string())
                    .attr("port_to", &link.port_to.to_string())
                    .attr("name", &link.name())
                    .attr("active", &link.active().to_string()),
            )?;
            writer.write(XMLWEvent::end_element())?;
        }
        writer.write(XMLWEvent::end_element())?;
        Ok(buffer)
    }

    /// Load the graph from a file with XML format
    ///
    pub fn load_from_xml(&self, buffer: Vec<u8>) -> anyhow::Result<()> {
        let private = imp::GraphView::from_obj(self);

        // Disable undo recording during file load
        private.undo_stack.borrow_mut().disable_recording();

        self.clear();
        let file = Cursor::new(buffer);
        let parser = EventReader::new(file);

        let mut current_node: Option<Node> = None;
        let mut current_node_properties: HashMap<String, String> = HashMap::new();
        let mut current_port: Option<Port> = None;
        let mut current_port_properties: HashMap<String, String> = HashMap::new();
        let mut current_link: Option<Link> = None;
        for e in parser {
            match e {
                Ok(XMLREvent::StartElement {
                    ref name,
                    ref attributes,
                    ..
                }) => {
                    trace!("Found XLM element={}", name);
                    let mut attrs = HashMap::new();
                    attributes.iter().for_each(|a| {
                        attrs.insert(a.name.to_string(), a.value.to_string());
                    });
                    match name.to_string().as_str() {
                        "Graph" => {
                            trace!("New graph detected");
                            if let Some(id) = attrs.get::<String>(&String::from("id")) {
                                self.set_id(id.parse::<u32>().expect("id should be an u32"));
                            }
                            if let Some(version) = attrs.get::<String>(&String::from("version")) {
                                info!("Found file format version: {}", version);
                            } else {
                                warn!("No file format version found");
                            }
                        }
                        "Node" => {
                            // Clear properties from any previous node before starting a new one
                            current_node_properties.clear();

                            let id = attrs
                                .get::<String>(&String::from("id"))
                                .expect("Unable to find node id");
                            let name = attrs
                                .get::<String>(&String::from("name"))
                                .expect("Unable to find node name");
                            let node_type: &String = attrs
                                .get::<String>(&String::from("type"))
                                .expect("Unable to find node type");
                            let default_value = String::from("0");
                            let pos_x: &String = attrs
                                .get::<String>(&String::from("pos_x"))
                                .unwrap_or(&default_value);
                            let pos_y: &String = attrs
                                .get::<String>(&String::from("pos_y"))
                                .unwrap_or(&default_value);
                            let default_value = String::from("false");
                            let light: &String = attrs
                                .get::<String>(&String::from("light"))
                                .unwrap_or(&default_value);
                            let node = self.create_node_with_id(
                                id.parse::<u32>().unwrap(),
                                name,
                                NodeType::from_str(node_type.as_str()),
                            );
                            node.set_position(
                                pos_x.parse::<f32>().unwrap(),
                                pos_y.parse::<f32>().unwrap(),
                            );
                            node.set_light(light.parse::<bool>().unwrap());
                            current_node = Some(node);
                        }
                        "Property" => {
                            let name = attrs
                                .get::<String>(&String::from("name"))
                                .expect("Unable to find property name");
                            let value: &String = attrs
                                .get::<String>(&String::from("value"))
                                .expect("Unable to find property value");
                            if current_port.is_some() {
                                current_port_properties.insert(name.to_string(), value.to_string());
                            } else if current_node.is_some() {
                                info!("add property to node {}={}", name, value);
                                current_node_properties.insert(name.to_string(), value.to_string());
                            }
                        }
                        "Port" => {
                            let id = attrs
                                .get::<String>(&String::from("id"))
                                .expect("Unable to find port id");
                            let name = attrs
                                .get::<String>(&String::from("name"))
                                .expect("Unable to find port name");
                            let direction: &String = attrs
                                .get::<String>(&String::from("direction"))
                                .expect("Unable to find port direction");
                            let default_value = PortPresence::Always.to_string();
                            let presence: &String = attrs
                                .get::<String>(&String::from("presence"))
                                .unwrap_or(&default_value);
                            current_port = Some(self.create_port_with_id(
                                id.parse::<u32>().unwrap(),
                                name,
                                PortDirection::from_str(direction),
                                PortPresence::from_str(presence),
                            ));
                        }
                        "Link" => {
                            let id = attrs
                                .get::<String>(&String::from("id"))
                                .expect("Unable to find link id");
                            let node_from = attrs
                                .get::<String>(&String::from("node_from"))
                                .expect("Unable to find link node_from");
                            let node_to = attrs
                                .get::<String>(&String::from("node_to"))
                                .expect("Unable to find link node_to");
                            let port_from = attrs
                                .get::<String>(&String::from("port_from"))
                                .expect("Unable to find link port_from");
                            let port_to = attrs
                                .get::<String>(&String::from("port_to"))
                                .expect("Unable to find link port_to");
                            let active: &String = attrs
                                .get::<String>(&String::from("active"))
                                .expect("Unable to find link state");
                            let default_value = String::from("");
                            let name: &String = attrs
                                .get::<String>(&String::from("name"))
                                .unwrap_or(&default_value);
                            let link = self.create_link_with_id(
                                id.parse::<u32>().unwrap(),
                                node_from.parse::<u32>().unwrap(),
                                node_to.parse::<u32>().unwrap(),
                                port_from.parse::<u32>().unwrap(),
                                port_to.parse::<u32>().unwrap(),
                            );

                            link.set_active(active.parse::<bool>().unwrap());
                            link.set_name(name.parse::<String>().unwrap().as_str());
                            current_link = Some(link);
                        }
                        _ => warn!("name unknown: {}", name),
                    }
                }
                Ok(XMLREvent::EndElement { name }) => {
                    trace!("closing {}", name);
                    match name.to_string().as_str() {
                        "Graph" => {
                            trace!("Graph ended with success");
                        }
                        "Node" => {
                            if let Some(node) = current_node {
                                let id = node.id();
                                let position =
                                    graphene::Point::new(node.position().0, node.position().1);
                                info!(
                                    "Applying {} properties to node id {} ({})",
                                    current_node_properties.len(),
                                    id,
                                    node.name()
                                );
                                node.update_properties(&current_node_properties);
                                current_node_properties.clear();
                                self.add_node(node);
                                if let Some(node) = self.node(id) {
                                    self.move_node(&node, &position);
                                }

                                self.update_current_node_id(id);
                            }
                            current_node = None;
                        }
                        "Property" => {}
                        "Port" => {
                            if let Some(port) = current_port {
                                if let Some(mut node) = current_node.clone() {
                                    let id = port.id();
                                    port.update_properties(&current_port_properties);
                                    self.add_port_to_node(&mut node, port);
                                    current_port_properties.clear();
                                    self.update_current_port_id(id);
                                }
                            }

                            current_port = None;
                        }
                        "Link" => {
                            if let Some(link) = current_link {
                                let id = link.id;
                                self.add_link(link);
                                self.update_current_link_id(id);
                            }
                            current_link = None;
                        }
                        _ => warn!("name unknown: {}", name),
                    }
                }
                Err(e) => {
                    error!("Error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        // Clear undo history and re-enable recording after file load
        private.undo_stack.borrow_mut().clear();
        private.undo_stack.borrow_mut().enable_recording();

        Ok(())
    }

    /// Load a graph from DOT format string.
    ///
    /// This method parses a DOT file and creates nodes, ports, and links.
    /// A `DotLoader` implementation provides domain-specific customization
    /// such as element type detection and static pad information.
    ///
    /// # Arguments
    /// * `content` - The DOT file content as a string
    /// * `loader` - A trait implementation providing domain-specific logic
    ///
    /// # Example
    /// ```ignore
    /// // Using a custom loader (e.g., for GStreamer)
    /// graphview.load_from_dot(dot_content, &GstDotLoader)?;
    /// ```
    pub fn load_from_dot<L: super::dot_parser::DotLoader>(
        &self,
        content: &str,
        loader: &L,
    ) -> anyhow::Result<()> {
        use super::dot_parser::DotGraph;

        let private = imp::GraphView::from_obj(self);

        // Parse DOT content first (before modifying state) to fail early on invalid input
        let dot_graph = DotGraph::parse(content, loader)?;

        // Disable undo recording during file load and clear the graph
        private.undo_stack.borrow_mut().disable_recording();
        self.clear();

        // Create context to hold shared state during DOT loading
        let mut ctx = DotLoadContext::new(&dot_graph, loader);

        // Phase 1: Create nodes from DOT elements
        let missing_elements = self.create_nodes_from_dot(&dot_graph, loader, &mut ctx);

        // Phase 2: Create ports (static "Always" ports, then dynamic "Sometimes" ports)
        self.create_ports_from_dot(&dot_graph, loader, &mut ctx);

        // Phase 3: Create links between ports
        let unresolved_links = self.create_links_from_dot(&dot_graph, &ctx);

        // Clear undo history and re-enable recording after file load
        private.undo_stack.borrow_mut().clear();
        private.undo_stack.borrow_mut().enable_recording();

        // Log summary
        if !missing_elements.is_empty() {
            warn!(
                "DOT import: {} element(s) not found in registry: {}",
                missing_elements.len(),
                missing_elements.join(", ")
            );
        }
        if unresolved_links > 0 {
            warn!(
                "DOT import: {} link(s) could not be created",
                unresolved_links
            );
        }

        info!(
            "Loaded DOT graph: {} nodes, {} links",
            ctx.node_id_map.len(),
            dot_graph.links.len().saturating_sub(unresolved_links)
        );

        Ok(())
    }

    /// Create nodes from DOT elements.
    ///
    /// Returns a list of element type names that were not found in the registry.
    fn create_nodes_from_dot<L: super::dot_parser::DotLoader>(
        &self,
        dot_graph: &super::dot_parser::DotGraph,
        loader: &L,
        ctx: &mut DotLoadContext,
    ) -> Vec<String> {
        let mut missing_elements: Vec<String> = Vec::new();

        for dot_node in &dot_graph.nodes {
            let type_name = &dot_node.type_name;

            // Use loader to determine node type
            let node_type = loader.node_type(type_name);

            // Create the node
            let node = self.create_node(type_name, node_type);
            let node_id = node.id();

            // Check if node type exists (using loader)
            if !loader.node_exists(type_name) {
                node.set_light(true);
                missing_elements.push(type_name.clone());
            }

            // Add metadata (filtered by the parser)
            for (key, value) in &dot_node.metadata {
                node.add_property(key, value);
            }

            // Store mapping
            ctx.node_id_map.insert(dot_node.dot_id.clone(), node_id);

            // Add the node to the graph
            self.add_node(node);

            debug!("Created node: {} (id={})", type_name, node_id);
        }

        missing_elements
    }

    /// Create ports from DOT elements.
    ///
    /// This is done in two passes:
    /// 1. Create static "Always" ports from loader's factory info
    /// 2. Create dynamic "Sometimes" ports for links that weren't mapped in pass 1
    fn create_ports_from_dot<L: super::dot_parser::DotLoader>(
        &self,
        dot_graph: &super::dot_parser::DotGraph,
        loader: &L,
        ctx: &mut DotLoadContext,
    ) {
        // Collect node info first to avoid borrow conflicts
        // (we need immutable access to links_by_instance while mutating port_id_map)
        let nodes_info: Vec<_> = dot_graph
            .nodes
            .iter()
            .filter_map(|dot_node| {
                ctx.node_id_map.get(&dot_node.dot_id).map(|&node_id| {
                    let normalized_instance = dot_node.instance_name.replace('-', "_");
                    let node_links = ctx
                        .links_by_instance
                        .get(&normalized_instance)
                        .cloned()
                        .unwrap_or_default();
                    (node_id, dot_node.type_name.clone(), node_links)
                })
            })
            .collect();

        // First pass: Create static "Always" ports from loader's factory info
        for (node_id, type_name, node_links) in &nodes_info {
            // Get static ports from loader
            let (inputs, outputs) = loader.get_static_ports(type_name);

            // Add input ports
            for input_name in &inputs {
                let port_id = self.create_and_map_port(
                    *node_id,
                    input_name,
                    PortDirection::Input,
                    PortPresence::Always,
                    &dot_graph.links,
                    node_links,
                    false, // sink ports
                    loader,
                    &mut ctx.port_id_map,
                    &mut ctx.node_for_port,
                );

                trace!(
                    "Created input port: {} on node {} (id={})",
                    input_name,
                    type_name,
                    port_id
                );
            }

            // Add output ports
            for output_name in &outputs {
                let port_id = self.create_and_map_port(
                    *node_id,
                    output_name,
                    PortDirection::Output,
                    PortPresence::Always,
                    &dot_graph.links,
                    node_links,
                    true, // source ports
                    loader,
                    &mut ctx.port_id_map,
                    &mut ctx.node_for_port,
                );

                trace!(
                    "Created output port: {} on node {} (id={})",
                    output_name,
                    type_name,
                    port_id
                );
            }
        }

        // Second pass: Create dynamic "Sometimes" ports for links that weren't mapped above
        for (node_id, type_name, node_links) in &nodes_info {
            // Check for unmapped ports in links belonging to this element
            for &(link_idx, is_source) in node_links {
                let link = &dot_graph.links[link_idx];
                let dot_port_id = if is_source {
                    &link.from_port_id
                } else {
                    &link.to_port_id
                };

                // Skip if already mapped
                if ctx.port_id_map.contains_key(dot_port_id) {
                    continue;
                }

                let port_name = loader
                    .extract_port_name_from_id(dot_port_id)
                    .unwrap_or_else(|| if is_source { "src" } else { "sink" }.to_string());

                let (direction, presence) = if is_source {
                    (PortDirection::Output, PortPresence::Sometimes)
                } else {
                    (PortDirection::Input, PortPresence::Sometimes)
                };

                let port = self.create_port(&port_name, direction, presence);
                let port_id = port.id();

                if let Some(mut node) = self.node(*node_id) {
                    self.add_port_to_node(&mut node, port);
                } else {
                    warn!(
                        "DOT import: could not find node {} to add dynamic port {}",
                        node_id, port_name
                    );
                    continue;
                }

                ctx.port_id_map.insert(dot_port_id.clone(), port_id);
                ctx.node_for_port.insert(dot_port_id.clone(), *node_id);

                trace!(
                    "Created dynamic {} port: {} on node {} (id={})",
                    if is_source { "output" } else { "input" },
                    port_name,
                    type_name,
                    port_id
                );
            }
        }
    }

    /// Create links between ports based on DOT edge information.
    ///
    /// Returns the number of links that could not be created.
    fn create_links_from_dot(
        &self,
        dot_graph: &super::dot_parser::DotGraph,
        ctx: &DotLoadContext,
    ) -> usize {
        let mut unresolved_links = 0;

        for link in &dot_graph.links {
            let port_from = ctx.port_id_map.get(&link.from_port_id);
            let port_to = ctx.port_id_map.get(&link.to_port_id);

            if let (Some(&port_from), Some(&port_to)) = (port_from, port_to) {
                if let (Some(&node_from), Some(&node_to)) = (
                    ctx.node_for_port.get(&link.from_port_id),
                    ctx.node_for_port.get(&link.to_port_id),
                ) {
                    let new_link = self.create_link(node_from, node_to, port_from, port_to);
                    new_link.set_active(true);
                    let link_id = new_link.id();

                    self.add_link(new_link);

                    trace!(
                        "Created link: {} -> {} (id={})",
                        link.from_port_id,
                        link.to_port_id,
                        link_id
                    );
                } else {
                    unresolved_links += 1;
                    warn!(
                        "Could not create link: {} -> {} (node lookup failed)",
                        link.from_port_id, link.to_port_id
                    );
                }
            } else {
                unresolved_links += 1;
                // More informative error message showing which port is missing
                let from_status = if port_from.is_some() {
                    "resolved"
                } else {
                    "MISSING"
                };
                let to_status = if port_to.is_some() {
                    "resolved"
                } else {
                    "MISSING"
                };
                warn!(
                    "Could not create link: from '{}' ({}) -> to '{}' ({})",
                    link.from_port_id, from_status, link.to_port_id, to_status
                );
            }
        }

        unresolved_links
    }

    /// Helper to create a port and map it to DOT link endpoints.
    #[allow(clippy::too_many_arguments)]
    fn create_and_map_port<L: super::dot_parser::DotLoader>(
        &self,
        node_id: u32,
        port_name: &str,
        direction: PortDirection,
        presence: PortPresence,
        links: &[super::dot_parser::DotLink],
        link_indices: &[(usize, bool)],
        is_source: bool,
        loader: &L,
        port_id_map: &mut HashMap<String, u32>,
        node_for_port: &mut HashMap<String, u32>,
    ) -> u32 {
        let port = self.create_port(port_name, direction, presence);
        let port_id = port.id();

        if let Some(mut node) = self.node(node_id) {
            self.add_port_to_node(&mut node, port);
        } else {
            warn!(
                "DOT import: could not find node {} to add port {}",
                node_id, port_name
            );
            // Return early - don't map ports that couldn't be attached to a node
            return port_id;
        }

        // Map DOT link endpoints for this port
        for &(link_idx, link_is_source) in link_indices {
            if link_is_source == is_source {
                let dot_port_id = if is_source {
                    &links[link_idx].from_port_id
                } else {
                    &links[link_idx].to_port_id
                };

                if let Some(dot_port_name) = loader.extract_port_name_from_id(dot_port_id) {
                    if dot_port_name == port_name {
                        port_id_map.insert(dot_port_id.clone(), port_id);
                        node_for_port.insert(dot_port_id.clone(), node_id);
                    }
                }
            }
        }

        port_id
    }

    //Private

    fn create_node_with_id(&self, id: u32, name: &str, node_type: NodeType) -> Node {
        Node::new(id, name, node_type)
    }

    fn create_port_with_id(
        &self,
        id: u32,
        name: &str,
        direction: PortDirection,
        presence: PortPresence,
    ) -> Port {
        Port::new(id, name, direction, presence)
    }

    fn create_link_with_id(
        &self,
        link_id: u32,
        node_from_id: u32,
        node_to_id: u32,
        port_from_id: u32,
        port_to_id: u32,
    ) -> Link {
        Link::new(link_id, node_from_id, node_to_id, port_from_id, port_to_id)
    }

    pub fn remove_link(&self, id: u32) {
        let private = imp::GraphView::from_obj(self);

        // Record undo action before removing
        if let Some(link) = private.links.borrow().get(&id) {
            private.undo_stack.borrow_mut().push(
                crate::graphmanager::undo::UndoAction::RemoveLink {
                    link_data: crate::graphmanager::undo::LinkData {
                        id: link.id,
                        node_from: link.node_from,
                        node_to: link.node_to,
                        port_from: link.port_from,
                        port_to: link.port_to,
                        active: link.active(),
                        name: link.name(),
                    },
                },
            );
        }

        let mut links = private.links.borrow_mut();
        links.remove(&id);
        drop(links); // Release borrow before emitting signal
        self.emit_by_name::<()>("link-removed", &[&private.id.get(), &id]);
        self.queue_draw();
    }

    fn update_current_link_id(&self, link_id: u32) {
        let private = imp::GraphView::from_obj(self);
        if link_id > private.current_link_id.get() {
            private.current_link_id.set(link_id);
        }
    }

    fn link_exists(&self, new_link: &Link) -> bool {
        let private = imp::GraphView::from_obj(self);

        for link in private.links.borrow().values() {
            if (new_link.port_from == link.port_from && new_link.port_to == link.port_to)
                || (new_link.port_to == link.port_from && new_link.port_from == link.port_to)
            {
                warn!("link already existing");
                return true;
            }
        }
        false
    }

    fn move_node(&self, widget: &Node, point: &graphene::Point) {
        let mut nodes = self.imp().nodes.borrow_mut();
        let node = nodes
            .get_mut(&widget.id())
            .expect("Node is not on the graph");
        node.0.set_position(point.x(), point.y());
        node.1 = graphene::Point::new(
            point.x().clamp(
                -(CANVAS_SIZE / 2.0) as f32,
                (CANVAS_SIZE / 2.0) as f32 - widget.width() as f32,
            ),
            point.y().clamp(
                -(CANVAS_SIZE / 2.0) as f32,
                (CANVAS_SIZE / 2.0) as f32 - widget.height() as f32,
            ),
        );

        // we don't need to redraw the full graph everytime.
        self.queue_allocate();
    }

    fn unselect_nodes(&self) {
        let private = imp::GraphView::from_obj(self);
        for node in private.nodes.borrow_mut().values() {
            node.0.set_selected(false);
            node.0.unselect_all_ports();
        }
    }

    fn update_current_node_id(&self, node_id: u32) {
        let private = imp::GraphView::from_obj(self);
        if node_id > private.current_node_id.get() {
            private.current_node_id.set(node_id);
        }
    }

    fn unselect_links(&self) {
        let private = imp::GraphView::from_obj(self);
        for link in private.links.borrow_mut().values() {
            link.set_selected(false);
        }
    }

    fn unselect_all(&self) {
        self.unselect_nodes();
        self.unselect_links();
        self.queue_draw();
    }

    fn point_on_link(&self, point: &graphene::Point) -> Option<Link> {
        let private = imp::GraphView::from_obj(self);
        self.unselect_all();
        for link in private.links.borrow_mut().values() {
            if let Some((from_x, from_y, to_x, to_y)) = private.link_coordinates(link) {
                let quad = graphene::Quad::new(
                    &graphene::Point::new(from_x as f32, from_y as f32 - link.thickness as f32),
                    &graphene::Point::new(to_x as f32, to_y as f32 - link.thickness as f32),
                    &graphene::Point::new(to_x as f32, to_y as f32 + link.thickness as f32),
                    &graphene::Point::new(from_x as f32, from_y as f32 + link.thickness as f32),
                );
                if quad.contains(point) {
                    link.toggle_selected();
                    self.queue_draw();
                    return Some(link.clone());
                }
            }
        }
        self.queue_draw();
        None
    }

    pub fn graph_updated(&self) {
        let private = imp::GraphView::from_obj(self);
        self.queue_allocate();
        self.emit_by_name::<()>("graph-updated", &[&private.id.get()]);
    }

    fn next_node_id(&self) -> u32 {
        let private = imp::GraphView::from_obj(self);
        private
            .current_node_id
            .set(private.current_node_id.get() + 1);
        private.current_node_id.get()
    }

    fn next_port_id(&self) -> u32 {
        let private = imp::GraphView::from_obj(self);
        private
            .current_port_id
            .set(private.current_port_id.get() + 1);
        private.current_port_id.get()
    }

    fn next_link_id(&self) -> u32 {
        let private = imp::GraphView::from_obj(self);
        private
            .current_link_id
            .set(private.current_link_id.get() + 1);
        private.current_link_id.get()
    }

    fn set_selected_port(&self, port: Option<&Port>) {
        if port.is_some() {
            self.unselect_all();
        }
        let private = imp::GraphView::from_obj(self);
        *private.port_selected.borrow_mut() = port.cloned();
    }

    fn selected_port(&self) -> RefMut<'_, Option<Port>> {
        let private = imp::GraphView::from_obj(self);
        private.port_selected.borrow_mut()
    }

    fn set_mouse_position(&self, x: f64, y: f64) {
        let private = imp::GraphView::from_obj(self);
        private.mouse_position.set((x, y));
    }

    fn ports_compatible(&self, to_port: &Port) -> bool {
        let current_port = self.selected_port().to_owned();
        if let Some(from_port) = current_port {
            let from_node = from_port
                .ancestor(Node::static_type())
                .expect("Unable to reach parent")
                .dynamic_cast::<Node>()
                .expect("Unable to cast to Node");
            let to_node = to_port
                .ancestor(Node::static_type())
                .expect("Unable to reach parent")
                .dynamic_cast::<Node>()
                .expect("Unable to cast to Node");
            let res = from_port.id() != to_port.id()
                && from_port.direction() != to_port.direction()
                && from_node.id() != to_node.id();
            if !res {
                warn!("Unable add the following link");
            }
            return res;
        }
        false
    }

    fn update_current_port_id(&self, port_id: u32) {
        let private = imp::GraphView::from_obj(self);
        if port_id > private.current_port_id.get() {
            private.current_port_id.set(port_id);
        }
    }
    fn set_adjustment(
        &self,
        obj: &super::GraphView,
        adjustment: Option<&gtk::Adjustment>,
        orientation: gtk::Orientation,
    ) {
        let private = imp::GraphView::from_obj(self);
        match orientation {
            gtk::Orientation::Horizontal => *private.hadjustment.borrow_mut() = adjustment.cloned(),
            gtk::Orientation::Vertical => *private.vadjustment.borrow_mut() = adjustment.cloned(),
            _ => unimplemented!(),
        }

        if let Some(adjustment) = adjustment {
            adjustment.connect_value_changed(clone!(
                #[weak]
                obj,
                move |_| obj.queue_allocate()
            ));
        }
    }

    fn set_adjustment_values(
        &self,
        obj: &super::GraphView,
        adjustment: &gtk::Adjustment,
        orientation: gtk::Orientation,
    ) {
        let private = imp::GraphView::from_obj(self);
        let size = match orientation {
            gtk::Orientation::Horizontal => obj.width(),
            gtk::Orientation::Vertical => obj.height(),
            _ => unimplemented!(),
        };
        let zoom_factor = private.zoom_factor.get();

        adjustment.configure(
            adjustment.value(),
            -(CANVAS_SIZE / 2.0) * zoom_factor,
            (CANVAS_SIZE / 2.0) * zoom_factor,
            (f64::from(size) * 0.1) * zoom_factor,
            (f64::from(size) * 0.9) * zoom_factor,
            f64::from(size) * zoom_factor,
        );
    }

    /// Scroll the view to center on the given position (in graph coordinates)
    pub fn scroll_to_position(&self, x: f32, y: f32) {
        let private = imp::GraphView::from_obj(self);
        let zoom_factor = private.zoom_factor.get();

        let hadjustment_ref = private.hadjustment.borrow();
        let vadjustment_ref = private.vadjustment.borrow();

        if let (Some(hadjustment), Some(vadjustment)) =
            (hadjustment_ref.as_ref(), vadjustment_ref.as_ref())
        {
            // Calculate the adjustment values to center the position in the viewport
            let view_width = self.width() as f64;
            let view_height = self.height() as f64;

            // Convert graph coordinates to screen coordinates and center
            let target_h = (x as f64 * zoom_factor) - (view_width / 2.0);
            let target_v = (y as f64 * zoom_factor) - (view_height / 2.0);

            hadjustment.set_value(target_h);
            vadjustment.set_value(target_v);
        }
    }

    // Undo/Redo API

    /// Undo the last action
    ///
    /// Returns true if an action was undone, false if there was nothing to undo
    pub fn undo(&self) -> bool {
        use crate::graphmanager::undo::UndoAction;

        let private = imp::GraphView::from_obj(self);

        // Disable recording and pop the action
        let action = {
            let mut undo_stack = private.undo_stack.borrow_mut();
            undo_stack.disable_recording();
            undo_stack.pop_undo()
        };

        let result = if let Some(action) = action {
            // Execute the reverse of the action
            match &action {
                UndoAction::AddNode { node_data, .. } => {
                    // Undo: Remove the node that was added
                    self.remove_node_internal(node_data.id);
                }
                UndoAction::RemoveNode {
                    node_data,
                    position,
                    connected_links,
                } => {
                    // Undo: Re-add the node that was removed
                    self.restore_node(node_data, position);
                    // Restore connected links
                    for link_data in connected_links {
                        self.restore_link(link_data);
                    }
                }
                UndoAction::AddLink { link_data } => {
                    // Undo: Remove the link that was added
                    self.remove_link_internal(link_data.id);
                }
                UndoAction::RemoveLink { link_data } => {
                    // Undo: Re-add the link that was removed
                    self.restore_link(link_data);
                }
                UndoAction::MoveNode {
                    node_id,
                    old_position,
                    ..
                } => {
                    // Undo: Move node back to old position
                    if let Some(node) = self.node(*node_id) {
                        self.move_node(&node, old_position);
                    }
                }
                UndoAction::AddPort { node_id, port_data } => {
                    // Undo: Remove the port that was added
                    self.remove_port(*node_id, port_data.id);
                }
                UndoAction::RemovePort { node_id, port_data } => {
                    // Undo: Re-add the port that was removed
                    if let Some(mut node) = self.node(*node_id) {
                        let port = self.restore_port(port_data);
                        self.add_port_to_node(&mut node, port);
                    }
                }
                UndoAction::ModifyProperty {
                    node_id,
                    port_id,
                    property_name,
                    old_value,
                    ..
                } => {
                    // Undo: Restore old property value
                    if let Some(node) = self.node(*node_id) {
                        if let Some(port_id) = port_id {
                            if let Some(port) = node.port(*port_id) {
                                if old_value.is_empty() {
                                    port.remove_property(property_name);
                                } else {
                                    port.add_property(property_name, old_value);
                                }
                            }
                        } else if old_value.is_empty() {
                            node.remove_property(property_name);
                        } else {
                            node.add_property(property_name, old_value);
                        }
                    }
                }
                UndoAction::BatchMoveNodes { moves } => {
                    // Undo: Move all nodes back to their old positions
                    for (node_id, old_position, _) in moves {
                        if let Some(node) = self.node(*node_id) {
                            self.move_node(&node, old_position);
                        }
                    }
                }
            }

            // Push the original action to redo stack so it can be redone
            private.undo_stack.borrow_mut().push_redo(action);
            private.undo_stack.borrow_mut().enable_recording();
            true
        } else {
            // Re-enable recording even if there was nothing to undo
            private.undo_stack.borrow_mut().enable_recording();
            false
        };

        if result {
            self.graph_updated();
        }

        result
    }

    /// Redo the last undone action
    ///
    /// Returns true if an action was redone, false if there was nothing to redo
    pub fn redo(&self) -> bool {
        use crate::graphmanager::undo::UndoAction;

        let private = imp::GraphView::from_obj(self);

        // Disable recording and pop the action from redo stack
        let action = {
            let mut undo_stack = private.undo_stack.borrow_mut();
            undo_stack.disable_recording();
            undo_stack.pop_redo()
        };

        let result = if let Some(action) = action {
            // Re-execute the original action
            match &action {
                UndoAction::AddNode {
                    node_data,
                    position,
                } => {
                    // Redo: Add the node back
                    self.restore_node(node_data, position);
                }
                UndoAction::RemoveNode { node_data, .. } => {
                    // Redo: Remove the node again
                    self.remove_node_internal(node_data.id);
                }
                UndoAction::AddLink { link_data } => {
                    // Redo: Add the link back
                    self.restore_link(link_data);
                }
                UndoAction::RemoveLink { link_data } => {
                    // Redo: Remove the link again
                    self.remove_link_internal(link_data.id);
                }
                UndoAction::MoveNode {
                    node_id,
                    new_position,
                    ..
                } => {
                    // Redo: Move to the new position
                    if let Some(node) = self.node(*node_id) {
                        self.move_node(&node, new_position);
                    }
                }
                UndoAction::AddPort { node_id, port_data } => {
                    // Redo: Add the port back
                    if let Some(mut node) = self.node(*node_id) {
                        let port = self.restore_port(port_data);
                        self.add_port_to_node(&mut node, port);
                    }
                }
                UndoAction::RemovePort { node_id, port_data } => {
                    // Redo: Remove the port again
                    self.remove_port(*node_id, port_data.id);
                }
                UndoAction::ModifyProperty {
                    node_id,
                    port_id,
                    property_name,
                    new_value,
                    ..
                } => {
                    // Redo: Apply the new value
                    if let Some(node) = self.node(*node_id) {
                        if let Some(port_id) = port_id {
                            if let Some(port) = node.port(*port_id) {
                                if new_value.is_empty() {
                                    port.remove_property(property_name);
                                } else {
                                    port.add_property(property_name, new_value);
                                }
                            }
                        } else if new_value.is_empty() {
                            node.remove_property(property_name);
                        } else {
                            node.add_property(property_name, new_value);
                        }
                    }
                }
                UndoAction::BatchMoveNodes { moves } => {
                    // Redo: Move all nodes to their new positions
                    for (node_id, _, new_position) in moves {
                        if let Some(node) = self.node(*node_id) {
                            self.move_node(&node, new_position);
                        }
                    }
                }
            }

            // Push the original action back to undo stack
            private.undo_stack.borrow_mut().push_undo(action);
            private.undo_stack.borrow_mut().enable_recording();
            true
        } else {
            // Re-enable recording even if there was nothing to redo
            private.undo_stack.borrow_mut().enable_recording();
            false
        };

        if result {
            self.graph_updated();
        }

        result
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        let private = imp::GraphView::from_obj(self);
        private.undo_stack.borrow().can_undo()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        let private = imp::GraphView::from_obj(self);
        private.undo_stack.borrow().can_redo()
    }

    /// Clear all undo/redo history
    pub fn clear_undo_history(&self) {
        let private = imp::GraphView::from_obj(self);
        private.undo_stack.borrow_mut().clear();
    }

    /// Set maximum undo depth
    pub fn set_max_undo_depth(&self, depth: usize) {
        let private = imp::GraphView::from_obj(self);
        private.undo_stack.borrow_mut().set_max_depth(depth);
    }

    /// Get number of actions in undo stack
    pub fn undo_count(&self) -> usize {
        let private = imp::GraphView::from_obj(self);
        private.undo_stack.borrow().undo_count()
    }

    /// Get number of actions in redo stack
    pub fn redo_count(&self) -> usize {
        let private = imp::GraphView::from_obj(self);
        private.undo_stack.borrow().redo_count()
    }

    /// Enable or disable undo recording
    pub fn set_undo_recording(&self, enabled: bool) {
        let private = imp::GraphView::from_obj(self);
        if enabled {
            private.undo_stack.borrow_mut().enable_recording();
        } else {
            private.undo_stack.borrow_mut().disable_recording();
        }
    }

    /// Modify a node property with undo support
    ///
    /// Records the old and new values so the change can be undone/redone
    pub fn modify_node_property(&self, node_id: u32, property_name: &str, new_value: &str) {
        if let Some(node) = self.node(node_id) {
            let old_value = PropertyExt::property(&node, property_name).unwrap_or_default();

            // Only record if the value actually changed
            if old_value != new_value {
                // If new value is empty, remove the property, otherwise add it
                if new_value.is_empty() {
                    node.remove_property(property_name);
                } else {
                    node.add_property(property_name, new_value);
                }

                let private = imp::GraphView::from_obj(self);
                private.undo_stack.borrow_mut().push(
                    crate::graphmanager::undo::UndoAction::ModifyProperty {
                        node_id,
                        port_id: None,
                        property_name: property_name.to_string(),
                        old_value,
                        new_value: new_value.to_string(),
                    },
                );
                self.graph_updated();
            }
        }
    }

    /// Modify a port property with undo support
    ///
    /// Records the old and new values so the change can be undone/redone
    pub fn modify_port_property(
        &self,
        node_id: u32,
        port_id: u32,
        property_name: &str,
        new_value: &str,
    ) {
        if let Some(node) = self.node(node_id) {
            if let Some(port) = node.port(port_id) {
                let old_value = PropertyExt::property(&port, property_name).unwrap_or_default();

                // Only record if the value actually changed
                if old_value != new_value {
                    // If new value is empty, remove the property, otherwise add it
                    if new_value.is_empty() {
                        port.remove_property(property_name);
                    } else {
                        port.add_property(property_name, new_value);
                    }

                    let private = imp::GraphView::from_obj(self);
                    private.undo_stack.borrow_mut().push(
                        crate::graphmanager::undo::UndoAction::ModifyProperty {
                            node_id,
                            port_id: Some(port_id),
                            property_name: property_name.to_string(),
                            old_value,
                            new_value: new_value.to_string(),
                        },
                    );
                    self.graph_updated();
                }
            }
        }
    }

    /// Update multiple node properties with undo support
    ///
    /// Each property change is recorded separately for granular undo/redo
    pub fn update_node_properties(&self, node_id: u32, properties: &HashMap<String, String>) {
        for (key, value) in properties {
            if value.is_empty() {
                // Handle removal - record as modification with empty value
                self.modify_node_property(node_id, key, "");
            } else {
                self.modify_node_property(node_id, key, value);
            }
        }
    }

    /// Update multiple port properties with undo support
    ///
    /// Each property change is recorded separately for granular undo/redo
    pub fn update_port_properties(
        &self,
        node_id: u32,
        port_id: u32,
        properties: &HashMap<String, String>,
    ) {
        for (key, value) in properties {
            if value.is_empty() {
                // Handle removal - record as modification with empty value
                self.modify_port_property(node_id, port_id, key, "");
            } else {
                self.modify_port_property(node_id, port_id, key, value);
            }
        }
    }

    // Auto-arrange methods

    /// Automatically arrange all nodes in the graph from source to sink.
    ///
    /// Groups nodes into stages (based on distance from sources) and
    /// distributes nodes vertically within each stage using barycenter ordering.
    ///
    /// The horizontal spacing between stages is dynamic: each stage starts after
    /// the widest node in the previous stage plus the configured `horizontal_spacing` gap.
    ///
    /// This operation is undoable as a single action.
    ///
    /// # Arguments
    /// * `options` - Optional layout configuration. Uses defaults if None.
    ///
    /// # Returns
    /// `true` if layout was applied, `false` if graph is empty.
    ///
    /// # Note
    /// Node widths are determined by GTK widget allocation. If called before nodes
    /// are realized (e.g., during file loading), widths may be 0 and stages will
    /// be spaced using only `horizontal_spacing`. For best results, call this method
    /// after the graph view has been displayed.
    pub fn auto_arrange_graph(&self, options: Option<AutoArrangeOptions>) -> bool {
        use std::collections::VecDeque;

        let options = options.unwrap_or_default();
        let private = imp::GraphView::from_obj(self);

        let nodes = self.all_nodes(NodeType::All);
        if nodes.is_empty() {
            return false;
        }

        // Build edge maps (with port index info for crossing minimization)
        let (forward_edges, backward_edges) = self.build_edge_maps();

        // Compute input count for each node
        let mut input_count: HashMap<u32, usize> = HashMap::new();
        for node in &nodes {
            input_count.insert(node.id(), 0);
        }
        for downstream_list in forward_edges.values() {
            for edge in downstream_list {
                *input_count.entry(edge.node_id).or_insert(0) += 1;
            }
        }

        // Find source nodes (input_count == 0)
        let sources: Vec<u32> = input_count
            .iter()
            .filter(|(_, &count)| count == 0)
            .map(|(&id, _)| id)
            .collect();

        // Assign each node to a stage (processing step in the pipeline, based on distance from sources)
        // using longest-path algorithm (modified Kahn's)
        let mut stage: HashMap<u32, usize> = HashMap::new();
        for node in &nodes {
            stage.insert(node.id(), 0);
        }

        let mut queue: VecDeque<u32> = sources.into_iter().collect();
        let mut remaining_input_count = input_count.clone();

        while let Some(node_id) = queue.pop_front() {
            let current_stage = *stage.get(&node_id).unwrap_or(&0);

            if let Some(downstream_list) = forward_edges.get(&node_id) {
                for edge in downstream_list {
                    // Update stage to maximum distance from sources
                    let new_stage = current_stage + 1;
                    stage
                        .entry(edge.node_id)
                        .and_modify(|s| *s = (*s).max(new_stage));

                    // Decrement input count and add to queue if ready
                    if let Some(count) = remaining_input_count.get_mut(&edge.node_id) {
                        *count = count.saturating_sub(1);
                        if *count == 0 {
                            queue.push_back(edge.node_id);
                        }
                    }
                }
            }
        }

        // Handle disconnected nodes (place them in an extra stage at the end)
        let max_stage = stage.values().max().copied().unwrap_or(0);
        for node in &nodes {
            stage.entry(node.id()).or_insert(max_stage + 1);
        }

        // Group nodes by stage
        let num_stages = stage.values().max().copied().unwrap_or(0) + 1;
        let mut stages: Vec<Vec<u32>> = vec![Vec::new(); num_stages];
        for (&node_id, &stage_idx) in &stage {
            if stage_idx < num_stages {
                stages[stage_idx].push(node_id);
            }
        }

        // Compute vertical positions using barycenter ordering with multiple passes
        let y_positions =
            self.compute_vertical_positions(&stages, &forward_edges, &backward_edges, &options);

        // Compute maximum width for each stage
        let mut stage_max_widths: Vec<f32> = vec![0.0; num_stages];
        for (stage_idx, stage_nodes) in stages.iter().enumerate() {
            for &node_id in stage_nodes {
                if let Some(node) = self.node(node_id) {
                    let node_width = node.width() as f32;
                    if node_width > stage_max_widths[stage_idx] {
                        stage_max_widths[stage_idx] = node_width;
                    }
                }
            }
        }

        // Compute cumulative X positions for each stage based on actual widths
        // Each stage starts after the previous stage's max width + horizontal_spacing gap
        let mut stage_x_positions: Vec<f32> = vec![options.start_x; num_stages];
        for i in 1..num_stages {
            stage_x_positions[i] =
                stage_x_positions[i - 1] + stage_max_widths[i - 1] + options.horizontal_spacing;
        }

        // Collect old positions and compute new positions
        let mut moves: Vec<(u32, graphene::Point, graphene::Point)> = Vec::new();

        for node in &nodes {
            let node_id = node.id();
            let stage_idx = stage.get(&node_id).copied().unwrap_or(0);
            let new_x = stage_x_positions
                .get(stage_idx)
                .copied()
                .unwrap_or(options.start_x);
            let new_y = y_positions
                .get(&node_id)
                .copied()
                .unwrap_or(options.start_y);

            // Get old position from the nodes HashMap
            if let Some((_, old_point)) = private.nodes.borrow().get(&node_id) {
                let old_pos = *old_point;
                let new_pos = graphene::Point::new(new_x, new_y);

                // Only record if position actually changed
                if (new_pos.x() - old_pos.x()).abs() > 0.1
                    || (new_pos.y() - old_pos.y()).abs() > 0.1
                {
                    moves.push((node_id, old_pos, new_pos));
                }
            }
        }

        // Apply new positions
        for (node_id, _, new_pos) in &moves {
            if let Some(node) = self.node(*node_id) {
                self.move_node(&node, new_pos);
            }
        }

        // Record batch undo action (single undo for entire layout)
        if !moves.is_empty() {
            private
                .undo_stack
                .borrow_mut()
                .push(crate::graphmanager::undo::UndoAction::BatchMoveNodes { moves });
        }

        // Center the view on the pipeline
        // Calculate bounding box of all nodes after arrangement
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        for node in &nodes {
            if let Some((_, point)) = private.nodes.borrow().get(&node.id()) {
                let node_width = node.width() as f32;
                let node_height = node.height() as f32;

                min_x = min_x.min(point.x());
                max_x = max_x.max(point.x() + node_width);
                min_y = min_y.min(point.y());
                max_y = max_y.max(point.y() + node_height);
            }
        }

        // Scroll to center of the pipeline
        if min_x != f32::MAX {
            let center_x = (min_x + max_x) / 2.0;
            let center_y = (min_y + max_y) / 2.0;
            self.scroll_to_position(center_x, center_y);
        }

        self.graph_updated();
        true
    }

    /// Build forward and backward edge maps from the links
    ///
    /// Returns (forward_edges, backward_edges) where each maps:
    /// - node_id -> list of (connected_node_id, port_index)
    ///
    /// The port_index helps order connections to minimize crossings
    fn build_edge_maps(&self) -> (HashMap<u32, Vec<EdgeInfo>>, HashMap<u32, Vec<EdgeInfo>>) {
        let private = imp::GraphView::from_obj(self);
        let links = private.links.borrow();
        let nodes = self.all_nodes(NodeType::All);

        let mut forward_edges: HashMap<u32, Vec<EdgeInfo>> = HashMap::new();
        let mut backward_edges: HashMap<u32, Vec<EdgeInfo>> = HashMap::new();

        // Initialize empty vectors for all nodes
        for node in &nodes {
            forward_edges.insert(node.id(), Vec::new());
            backward_edges.insert(node.id(), Vec::new());
        }

        // Build a map of port_id -> port_index for each node
        // Port index is the position of the port in the node's port list (for input or output)
        let mut port_indices: HashMap<u32, usize> = HashMap::new();
        for node in &nodes {
            let mut input_idx = 0usize;
            let mut output_idx = 0usize;
            for port in node.all_ports(PortDirection::All) {
                match port.direction() {
                    PortDirection::Input => {
                        port_indices.insert(port.id(), input_idx);
                        input_idx += 1;
                    }
                    PortDirection::Output => {
                        port_indices.insert(port.id(), output_idx);
                        output_idx += 1;
                    }
                    _ => {}
                }
            }
        }

        // Build edges from links with port index info
        for link in links.values() {
            // For forward edges: use port_to index (which port on destination)
            let port_to_idx = port_indices.get(&link.port_to).copied().unwrap_or(0);
            forward_edges
                .entry(link.node_from)
                .or_default()
                .push(EdgeInfo {
                    node_id: link.node_to,
                    port_index: port_to_idx,
                });

            // For backward edges: use port_from index (which port on source)
            let port_from_idx = port_indices.get(&link.port_from).copied().unwrap_or(0);
            backward_edges
                .entry(link.node_to)
                .or_default()
                .push(EdgeInfo {
                    node_id: link.node_from,
                    port_index: port_from_idx,
                });
        }

        (forward_edges, backward_edges)
    }

    /// Compute vertical positions for nodes using iterative barycenter ordering
    ///
    /// Uses multiple passes (forward and backward sweeps) to minimize edge crossings.
    /// Each pass reorders nodes within stages based on the average Y position of
    /// their connected nodes in adjacent stages, taking port indices into account.
    fn compute_vertical_positions(
        &self,
        stages: &[Vec<u32>],
        forward_edges: &HashMap<u32, Vec<EdgeInfo>>,
        backward_edges: &HashMap<u32, Vec<EdgeInfo>>,
        options: &AutoArrangeOptions,
    ) -> HashMap<u32, f32> {
        if stages.is_empty() {
            return HashMap::new();
        }

        // Store the ordering of nodes within each stage (indices determine Y position)
        let mut stage_orderings: Vec<Vec<u32>> = stages.to_vec();

        // Initial assignment: first stage keeps original order
        // Subsequent stages ordered by upstream barycenter
        for stage_idx in 1..stage_orderings.len() {
            let adjacent = stage_orderings[stage_idx - 1].clone();
            self.reorder_stage_by_barycenter(
                &mut stage_orderings[stage_idx],
                backward_edges,
                &adjacent,
                options,
            );
        }

        // Perform multiple passes to refine the ordering
        for _ in 0..options.barycenter_iterations {
            // Backward pass (right to left): reorder based on downstream nodes
            for stage_idx in (0..stage_orderings.len().saturating_sub(1)).rev() {
                let adjacent = stage_orderings[stage_idx + 1].clone();
                self.reorder_stage_by_barycenter(
                    &mut stage_orderings[stage_idx],
                    forward_edges,
                    &adjacent,
                    options,
                );
            }

            // Forward pass (left to right): reorder based on upstream nodes
            for stage_idx in 1..stage_orderings.len() {
                let adjacent = stage_orderings[stage_idx - 1].clone();
                self.reorder_stage_by_barycenter(
                    &mut stage_orderings[stage_idx],
                    backward_edges,
                    &adjacent,
                    options,
                );
            }
        }

        // Convert orderings to Y positions
        let mut y_positions: HashMap<u32, f32> = HashMap::new();
        for stage_nodes in &stage_orderings {
            for (i, &node_id) in stage_nodes.iter().enumerate() {
                let y = options.start_y + (i as f32 * options.vertical_spacing);
                y_positions.insert(node_id, y);
            }
        }

        y_positions
    }

    /// Reorder nodes in a stage based on barycenter of connected nodes in adjacent stage
    ///
    /// The port_index in EdgeInfo is used to adjust the effective Y position:
    /// - Connections to higher-indexed ports result in slightly higher Y positions
    /// - This helps minimize edge crossings when multiple nodes connect to the same destination
    fn reorder_stage_by_barycenter(
        &self,
        stage: &mut [u32],
        edges: &HashMap<u32, Vec<EdgeInfo>>,
        adjacent_stage: &[u32],
        options: &AutoArrangeOptions,
    ) {
        // Build a position map for the adjacent stage
        let adjacent_positions: HashMap<u32, f32> = adjacent_stage
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, options.start_y + (i as f32 * options.vertical_spacing)))
            .collect();

        // Port index offset: fraction of vertical spacing to add per port index
        // This ensures that nodes connecting to port 0 appear above those connecting to port 1, etc.
        let port_offset = options.vertical_spacing * options.port_offset_factor;

        // Compute barycenter for each node in the stage
        let mut nodes_with_barycenter: Vec<(u32, f32)> = stage
            .iter()
            .map(|&node_id| {
                let connected = edges.get(&node_id).map(|v| v.as_slice()).unwrap_or(&[]);
                let barycenter = if connected.is_empty() {
                    // No connections: keep current relative position
                    f32::MAX
                } else {
                    let (sum, count) = connected.iter().fold((0.0f32, 0usize), |(s, c), edge| {
                        if let Some(&pos) = adjacent_positions.get(&edge.node_id) {
                            // Add port index offset to distinguish connections to different ports
                            let effective_pos = pos + (edge.port_index as f32 * port_offset);
                            (s + effective_pos, c + 1)
                        } else {
                            (s, c)
                        }
                    });
                    if count > 0 {
                        sum / count as f32
                    } else {
                        f32::MAX
                    }
                };
                (node_id, barycenter)
            })
            .collect();

        // Sort by barycenter, maintaining relative order for nodes with no connections
        nodes_with_barycenter.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        // Update the stage ordering
        for (i, (node_id, _)) in nodes_with_barycenter.into_iter().enumerate() {
            stage[i] = node_id;
        }
    }

    // Helper methods for undo/redo

    /// Remove node without recording undo action
    fn remove_node_internal(&self, id: u32) {
        let private = imp::GraphView::from_obj(self);

        if let Some(node) = private.nodes.borrow_mut().remove(&id) {
            while let Some(link_id) = self.node_is_linked(node.0.id()) {
                private.links.borrow_mut().remove(&link_id);
            }
            node.0.unparent();
        }
    }

    /// Remove link without recording undo action
    fn remove_link_internal(&self, id: u32) {
        let private = imp::GraphView::from_obj(self);
        let mut links = private.links.borrow_mut();
        links.remove(&id);
        drop(links); // Release borrow before emitting signal
        self.emit_by_name::<()>("link-removed", &[&private.id.get(), &id]);
        self.queue_draw();
    }

    /// Restore a node from NodeData
    fn restore_node(
        &self,
        node_data: &crate::graphmanager::undo::NodeData,
        position: &graphene::Point,
    ) {
        let node =
            self.create_node_with_id(node_data.id, &node_data.name, node_data.node_type.clone());
        node.set_position(node_data.position.0, node_data.position.1);
        node.set_light(node_data.light);
        node.set_unique_name(&node_data.unique_name);
        node.update_properties(&node_data.properties);

        // Add ports
        for port_data in &node_data.ports {
            let port = self.restore_port(port_data);
            let mut node_mut = node.clone();
            node_mut.add_port(port);
        }

        // Add node to graph
        let private = imp::GraphView::from_obj(self);
        private
            .nodes
            .borrow_mut()
            .insert(node.id(), (node.clone(), *position));
        node.set_parent(self);
        self.update_current_node_id(node_data.id);
    }

    /// Restore a port from PortData
    fn restore_port(&self, port_data: &crate::graphmanager::undo::PortData) -> Port {
        let port = self.create_port_with_id(
            port_data.id,
            &port_data.name,
            port_data.direction,
            port_data.presence,
        );
        port.update_properties(&port_data.properties);
        self.update_current_port_id(port_data.id);
        port
    }

    /// Restore a link from LinkData
    fn restore_link(&self, link_data: &crate::graphmanager::undo::LinkData) {
        let link = self.create_link_with_id(
            link_data.id,
            link_data.node_from,
            link_data.node_to,
            link_data.port_from,
            link_data.port_to,
        );
        link.set_active(link_data.active);
        link.set_name(&link_data.name);

        let private = imp::GraphView::from_obj(self);
        private.links.borrow_mut().insert(link.id, link);
        self.update_current_link_id(link_data.id);
    }
}

impl Default for GraphView {
    fn default() -> Self {
        Self::new()
    }
}
