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
    }

    #[derive(Default)]
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
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GraphView {
        const NAME: &'static str = "GraphView";
        type Type = super::GraphView;
        type ParentType = gtk::Widget;
        type Interfaces = (gtk::Scrollable,);

        fn class_init(klass: &mut Self::Class) {
            // The layout manager determines how child widgets are laid out.
            //klass.set_layout_manager_type::<gtk::FixedLayout>();
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
                    })
                } else {
                    None
                }
            });
            drag_controller.connect_drag_update(|drag_controller, x, y| {
                let widget = drag_controller
                    .widget()
                    .dynamic_cast::<super::GraphView>()
                    .expect("drag-update event is not on the GraphView");
                let dragged_node = widget.imp().dragged_node.borrow();
                let Some(DragState { node, offset }) = dragged_node.as_ref() else {
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
                    .dynamic_cast::<super::GraphView>()
                    .expect("drag-update event is not on the GraphView");
                widget.graph_updated();
            });

            let gesture = gtk::GestureClick::new();
            gesture.set_button(0);
            gesture.connect_pressed(
                clone!(@weak obj, @weak drag_controller => move |gesture, _n_press, x, y| {
                    if gesture.current_button() == gdk::BUTTON_SECONDARY {
                        let widget = drag_controller.widget()
                        .dynamic_cast::<Self::Type>()
                        .expect("click event is not on the GraphView");
                        let target = widget.pick(x, y, gtk::PickFlags::DEFAULT).expect("port pick() did not return a widget");
                        if let Some(target) = target.ancestor(Port::static_type()) {
                            let port = target.dynamic_cast::<Port>().expect("click event is not on the Port");
                            let node = port.ancestor(Node::static_type()).expect("Unable to reach parent").dynamic_cast::<Node>().expect("Unable to cast to Node");
                            obj.emit_by_name::<()>("port-right-clicked", &[&port.id(), &node.id(), &graphene::Point::new(x as f32,y as f32)]);
                        } else if let Some(target) = target.ancestor(Node::static_type()) {
                            let node = target.dynamic_cast::<Node>().expect("click event is not on the Node");
                            widget.unselect_all();
                            node.set_selected(true);
                            obj.emit_by_name::<()>("node-right-clicked", &[&node.id(), &graphene::Point::new(x as f32,y as f32)]);
                        } else {
                            widget.unselect_all();
                            obj.emit_by_name::<()>("graph-right-clicked", &[&graphene::Point::new(x as f32,y as f32)]);
                        }
                    } else if gesture.current_button() == gdk::BUTTON_PRIMARY {
                        let widget = drag_controller.widget()
                        .dynamic_cast::<Self::Type>()
                        .expect("click event is not on the GraphView");
                        let target = widget.pick(x, y, gtk::PickFlags::DEFAULT).expect("port pick() did not return a widget");
                        if let Some(target) = target.ancestor(Port::static_type()) {
                            let port = target.dynamic_cast::<Port>().expect("click event is not on the Node");
                            widget.unselect_all();
                            port.toggle_selected();
                        } else  if let Some(target) = target.ancestor(Node::static_type()) {
                            let node = target.dynamic_cast::<Node>().expect("click event is not on the Node");
                            widget.unselect_all();
                            node.toggle_selected();
                        }
                         else {
                            widget.point_on_link(&graphene::Point::new(x.floor() as f32,y.floor() as f32));
                        }
                    }
                }),
            );

            gesture.connect_released(clone!(@weak gesture, @weak obj, @weak drag_controller => move |_gesture, _n_press, x, y| {
                if gesture.current_button() == gdk::BUTTON_PRIMARY {
                    let widget = drag_controller
                            .widget()
                            .dynamic_cast::<Self::Type>()
                            .expect("click event is not on the GraphView");
                    if let Some(target) = widget.pick(x, y, gtk::PickFlags::DEFAULT) {
                        if let Some(target) = target.ancestor(Port::static_type()) {
                            let port_clicked = target.dynamic_cast::<Port>().expect("click event is not on the Port");
                            if widget.port_is_linked(port_clicked.id()).is_none() {
                                let selected_port = widget.selected_port().to_owned();
                                if let Some(mut port_from) = selected_port {
                                    debug!("Port {} is clicked at {}:{}", port_clicked.id(), x, y);
                                    let mut port_to = port_clicked;
                                    if widget.ports_compatible(&port_to) {
                                        let mut node_from = port_from.ancestor(Node::static_type()).expect("Unable to reach parent").dynamic_cast::<Node>().expect("Unable to cast to Node");
                                        let mut node_to = port_to.ancestor(Node::static_type()).expect("Unable to reach parent").dynamic_cast::<Node>().expect("Unable to cast to Node");
                                        info!("add link from port {} to {} ", port_from.id(), port_to.id());
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
                        }
                        else {
                            if let Some(target) = target.ancestor(Node::static_type()) {
                                let node = target.dynamic_cast::<Node>().expect("click event is not on the Node");
                                info!(" node id {}", node.id());
                                if _n_press % 2 == 0  {
                                    info!("double clicked node id {}", node.id());
                                    obj.emit_by_name::<()>("node-double-clicked", &[&node.id(), &graphene::Point::new(x as f32,y as f32)]);
                                }
                            } else if _n_press % 2 == 0  {
                                if let Some(link) = widget.point_on_link(&graphene::Point::new(x.floor() as f32,y.floor() as f32)) {
                                    info!("double clicked link id {}", link.id());
                                    obj.emit_by_name::<()>("link-double-clicked", &[&link.id(), &graphene::Point::new(x as f32,y as f32)]);
                                }
                            } else {
                                info!("double click {}",widget.width());
                            }

                            // Click to something else than a port
                            widget.set_selected_port(None);
                        }
                    }
                }
            }));
            obj.add_controller(drag_controller);
            obj.add_controller(gesture);

            let event_motion = gtk::EventControllerMotion::new();
            event_motion.connect_motion(glib::clone!(@weak obj => move |_e, x, y| {
                let graphview = obj;
                if graphview.selected_port().is_some() {
                    graphview.set_mouse_position(x,y);
                    graphview.queue_allocate();
                }

            }));
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
                    Signal::builder("link-double-clicked")
                        .param_types([u32::static_type(), graphene::Point::static_type()])
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
                .unwrap_or_else(|| (panic!("Unable to get node from {}", node_from)));

            let from_port = from_node
                .0
                .port(port_from)
                .unwrap_or_else(|| panic!("Unable to get port from {}", port_from));

            let (x, y) = from_port
                .translate_coordinates(
                    widget,
                    (from_port.width() / 2) as f64,
                    (from_port.height() / 2) as f64,
                )
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
            let (x, y) = to_port
                .translate_coordinates(
                    widget,
                    (to_port.width() / 2) as f64,
                    (to_port.height() / 2) as f64,
                )
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
            if selected {
                link_cr.set_source_rgb(1.0, 0.18, 0.18);
            } else {
                link_cr.set_source_rgb(0.0, 0.0, 0.0);
            }

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
        @extends gtk::Widget;
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

    /// Clear the graphview
    ///
    pub fn clear(&self) {
        self.remove_all_nodes();
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
            .map(|node| {
                // Map nodes to their locations
                let point = self.node_position(&node.0.clone().upcast()).unwrap();
                (point.x(), point.y())
            })
            .filter(|(x2, _)| {
                // Only look for other nodes that have a similar x coordinate
                (x - x2).abs() < 50.0
            })
            .max_by(|y1, y2| {
                // Get max in column
                y1.partial_cmp(y2).unwrap_or(Ordering::Equal)
            })
            .map_or(20_f32, |(_x, y)| y + 120.0);

        let node_id = node.id();
        private
            .nodes
            .borrow_mut()
            .insert(node.id(), (node, graphene::Point::new(x, y)));
        self.emit_by_name::<()>("node-added", &[&private.id.get(), &node_id]);
        self.graph_updated();
    }

    /// Remove node from the graphview
    ///
    pub fn remove_node(&self, id: u32) {
        let private = imp::GraphView::from_obj(self);

        if let Some(node) = private.nodes.borrow_mut().remove(&id) {
            while let Some(link_id) = self.node_is_linked(node.0.id()) {
                info!("Remove link id {}", link_id);
                private.links.borrow_mut().remove(&link_id);
            }
            node.0.unparent();
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
            private.links.borrow_mut().insert(link.id, link);
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
            for port in node.ports().values() {
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
                            if let Some(version) = attrs.get::<String>(&"version".to_string()) {
                                info!("Found file format version: {}", version);
                            } else {
                                warn!("No file format version found");
                            }
                        }
                        "Node" => {
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
        Ok(())
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

    fn remove_link(&self, id: u32) {
        let private = imp::GraphView::from_obj(self);
        let mut links = private.links.borrow_mut();
        links.remove(&id);

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

    fn graph_updated(&self) {
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

    fn selected_port(&self) -> RefMut<Option<Port>> {
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
            adjustment.connect_value_changed(clone!(@weak obj => move |_|  obj.queue_allocate() ));
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
}

impl Default for GraphView {
    fn default() -> Self {
        Self::new()
    }
}
