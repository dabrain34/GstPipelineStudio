// graphview.rs
//
// Copyright 2021 Tom A. Wagner <tom.a.wagner@protonmail.com>
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

extern crate xml;
use xml::reader::EventReader;
use xml::reader::XmlEvent as XMLREvent;
use xml::writer::EmitterConfig;
use xml::writer::XmlEvent as XMLWEvent;

use super::{node::Node, node::NodeType, port::Port, port::PortDirection};
use std::fs::File;
use std::io::BufReader;

use gtk::{
    glib::{self, clone},
    graphene, gsk,
    prelude::*,
    subclass::prelude::*,
};
use log::{error, warn};

use std::cell::RefMut;
use std::{cmp::Ordering, collections::HashMap, error};
#[derive(Debug, Clone)]
pub struct NodeLink {
    pub id: u32,
    pub node_from: u32,
    pub node_to: u32,
    pub port_from: u32,
    pub port_to: u32,
    pub active: bool,
}

mod imp {
    use super::*;

    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    use log::warn;

    #[derive(Default)]
    pub struct GraphView {
        pub(super) nodes: RefCell<HashMap<u32, Node>>,
        pub(super) links: RefCell<HashMap<u32, NodeLink>>,
        pub(super) current_node_id: Cell<u32>,
        pub(super) current_port_id: Cell<u32>,
        pub(super) current_link_id: Cell<u32>,
        pub(super) port_selected: RefCell<Option<Port>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GraphView {
        const NAME: &'static str = "GraphView";
        type Type = super::GraphView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            // The layout manager determines how child widgets are laid out.
            klass.set_layout_manager_type::<gtk::FixedLayout>();
            klass.set_css_name("graphview");
        }
    }

    impl ObjectImpl for GraphView {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let drag_state = Rc::new(RefCell::new(None));
            let drag_controller = gtk::GestureDrag::new();

            drag_controller.connect_drag_begin(
                clone!(@strong drag_state => move |drag_controller, x, y| {
                    let mut drag_state = drag_state.borrow_mut();
                    let widget = drag_controller
                        .widget()
                        .expect("drag-begin event has no widget")
                        .dynamic_cast::<Self::Type>()
                        .expect("drag-begin event is not on the GraphView");
                    // pick() should at least return the widget itself.
                    let target = widget.pick(x, y, gtk::PickFlags::DEFAULT).expect("drag-begin pick() did not return a widget");
                    *drag_state = if target.ancestor(Port::static_type()).is_some() {
                        // The user targeted a port, so the dragging should be handled by the Port
                        // component instead of here.
                        None
                    } else if let Some(target) = target.ancestor(Node::static_type()) {
                        // The user targeted a Node without targeting a specific Port.
                        // Drag the Node around the screen.
                        if let Some((x, y)) = widget.node_position(&target) {
                            Some((target, x, y))
                        } else {
                            error!("Failed to obtain position of dragged node, drag aborted.");
                            None
                        }
                    } else {
                        None
                    }
                }
            ));
            drag_controller.connect_drag_update(
                clone!(@strong drag_state => move |drag_controller, x, y| {
                    let widget = drag_controller
                        .widget()
                        .expect("drag-update event has no widget")
                        .dynamic_cast::<Self::Type>()
                        .expect("drag-update event is not on the GraphView");
                    let drag_state = drag_state.borrow();
                    if let Some((ref node, x1, y1)) = *drag_state {
                        widget.move_node(node, x1 + x as f32, y1 + y as f32);
                    }
                }
                ),
            );
            obj.add_controller(&drag_controller);

            let gesture = gtk::GestureClick::new();
            gesture.connect_released(clone!(@weak gesture => move |_gesture, _n_press, x, y| {
                let widget = drag_controller
                        .widget()
                        .expect("click event has no widget")
                        .dynamic_cast::<Self::Type>()
                        .expect("click event is not on the GraphView");
                        let target = widget.pick(x, y, gtk::PickFlags::DEFAULT).expect("port pick() did not return a widget");
                        if let Some(target) = target.ancestor(Port::static_type()) {
                            let to_port = target.dynamic_cast::<Port>().expect("click event is not on the Port");
                            if let None = widget.port_is_linked(to_port.id()) {
                                let selected_port = widget.selected_port().to_owned();
                                if let Some(from_port) = selected_port {
                                    println!("Port {} is clicked at {}:{}", to_port.id(), x, y);
                                    if widget.ports_compatible(&to_port) {
                                        let from_node = from_port.ancestor(Node::static_type()).expect("Unable to reach parent").dynamic_cast::<Node>().expect("Unable to cast to Node");
                                        let to_node = to_port.ancestor(Node::static_type()).expect("Unable to reach parent").dynamic_cast::<Node>().expect("Unable to cast to Node");
                                        println!("add link");
                                        widget.add_link(NodeLink {
                                            id: widget.next_link_id(),
                                            node_from: from_node.id(),
                                            node_to: to_node.id(),
                                            port_from: from_port.id(),
                                            port_to: to_port.id(),
                                            active: true
                                        } );
                                    }
                                    widget.set_selected_port(None);
                                } else {
                                    println!("add selected port id");
                                    widget.set_selected_port(Some(&to_port));
                                }
                            }
                        }
            }));
            obj.add_controller(&gesture);
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.nodes
                .borrow()
                .values()
                .for_each(|node| node.unparent())
        }
    }

    impl WidgetImpl for GraphView {
        fn snapshot(&self, widget: &Self::Type, snapshot: &gtk::Snapshot) {
            /* FIXME: A lot of hardcoded values in here.
            Try to use relative units (em) and colours from the theme as much as possible. */

            let alloc = widget.allocation();

            // Draw all children
            self.nodes
                .borrow()
                .values()
                .for_each(|node| self.instance().snapshot_child(node, snapshot));

            // Draw all links
            let link_cr = snapshot
                .append_cairo(&graphene::Rect::new(
                    0.0,
                    0.0,
                    alloc.width as f32,
                    alloc.height as f32,
                ))
                .expect("Failed to get cairo context");

            link_cr.set_line_width(1.5);

            for link in self.links.borrow().values() {
                if let Some((from_x, from_y, to_x, to_y)) = self.link_coordinates(link) {
                    //println!("from_x: {} from_y: {} to_x: {} to_y: {}", from_x, from_y, to_x, to_y);

                    // Use dashed line for inactive links, full line otherwise.
                    if link.active {
                        link_cr.set_dash(&[], 0.0);
                    } else {
                        link_cr.set_dash(&[10.0, 5.0], 0.0);
                    }

                    link_cr.move_to(from_x, from_y);
                    link_cr.line_to(to_x, to_y);
                    link_cr.set_line_width(2.0);

                    if let Err(e) = link_cr.stroke() {
                        warn!("Failed to draw graphview links: {}", e);
                    };
                } else {
                    warn!("Could not get allocation of ports of link: {:?}", link);
                }
            }
        }
    }

    impl GraphView {
        /// Get coordinates for the drawn link to start at and to end at.
        ///
        /// # Returns
        /// `Some((from_x, from_y, to_x, to_y))` if all objects the links refers to exist as widgets.
        fn link_coordinates(&self, link: &NodeLink) -> Option<(f64, f64, f64, f64)> {
            let nodes = self.nodes.borrow();

            // For some reason, gtk4::WidgetExt::translate_coordinates gives me incorrect values,
            // so we manually calculate the needed offsets here.

            let from_port = &nodes.get(&link.node_from)?.port(&link.port_from)?;
            let gtk::Allocation {
                x: mut fx,
                y: mut fy,
                width: fw,
                height: fh,
            } = from_port.allocation();

            let from_node = from_port
                .ancestor(Node::static_type())
                .expect("Port is not a child of a node");
            let gtk::Allocation { x: fnx, y: fny, .. } = from_node.allocation();
            fx += fnx + (fw / 2);
            fy += fny + (fh / 2);

            let to_port = &nodes.get(&link.node_to)?.port(&link.port_to)?;
            let gtk::Allocation {
                x: mut tx,
                y: mut ty,
                width: tw,
                height: th,
                ..
            } = to_port.allocation();
            let to_node = to_port
                .ancestor(Node::static_type())
                .expect("Port is not a child of a node");
            let gtk::Allocation { x: tnx, y: tny, .. } = to_node.allocation();
            tx += tnx + (tw / 2);
            ty += tny + (th / 2);

            Some((fx.into(), fy.into(), tx.into(), ty.into()))
        }
    }
}

glib::wrapper! {
    pub struct GraphView(ObjectSubclass<imp::GraphView>)
        @extends gtk::Widget;
}

impl GraphView {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create GraphView")
    }

    pub fn add_node_with_port(&self, id: u32, node: Node, input: u32, output: u32) {
        let private = imp::GraphView::from_instance(self);
        node.set_parent(self);

        // Place widgets in colums of 3, growing down
        let x = if let Some(node_type) = node.node_type() {
            match node_type {
                NodeType::Source => 20.0,
                NodeType::Transform => 220.0,
                NodeType::Sink => 420.0,
                _ => 20.0,
            }
        } else {
            420.0
        };

        let y = private
            .nodes
            .borrow()
            .values()
            .filter_map(|node| {
                // Map nodes to locations, discard nodes without location
                self.node_position(&node.clone().upcast())
            })
            .filter(|(x2, _)| {
                // Only look for other nodes that have a similar x coordinate
                (x - x2).abs() < 50.0
            })
            .max_by(|y1, y2| {
                // Get max in column
                y1.partial_cmp(y2).unwrap_or(Ordering::Equal)
            })
            .map_or(20_f32, |(_x, y)| y + 100.0);

        self.move_node(&node.clone().upcast(), x, y);

        private.nodes.borrow_mut().insert(id, node);
        let _i = 0;
        for _i in 0..input {
            let port_id = self.next_port_id();
            let port = Port::new(port_id, "in", PortDirection::Input);
            self.add_port(id, port_id, port);
        }

        let _i = 0;
        for _i in 0..output {
            let port_id = self.next_port_id();
            let port = Port::new(port_id, "out", PortDirection::Output);
            self.add_port(id, port_id, port);
        }
    }

    pub fn add_node(&self, id: u32, node: Node) {
        self.add_node_with_port(id, node, 0, 0);
    }

    pub fn remove_node(&self, id: u32) {
        let private = imp::GraphView::from_instance(self);
        let mut nodes = private.nodes.borrow_mut();
        if let Some(node) = nodes.remove(&id) {
            node.unparent();
        } else {
            warn!("Tried to remove non-existant node (id={}) from graph", id);
        }
        self.queue_draw();
    }
    pub fn all_nodes(&self) -> Vec<Node> {
        let private = imp::GraphView::from_instance(self);
        let nodes = private.nodes.borrow();
        let nodes_list: Vec<_> = nodes.iter().map(|(_, node)| node.clone()).collect();
        nodes_list
    }

    pub fn remove_all_nodes(&self) {
        let private = imp::GraphView::from_instance(self);
        let nodes_list = self.all_nodes();
        for node in nodes_list {
            if let Some(link_id) = self.node_is_linked(node.id()) {
                let mut links = private.links.borrow_mut();
                links.remove(&link_id);
            }
            self.remove_node(node.id());
        }
        private.current_node_id.set(0);
        private.current_port_id.set(0);
        private.current_link_id.set(0);
        self.queue_draw();
    }

    pub fn node_is_linked(&self, node_id: u32) -> Option<u32> {
        let private = imp::GraphView::from_instance(self);
        for (key, link) in private.links.borrow().iter() {
            if link.node_from == node_id || link.node_to == node_id {
                return Some(*key);
            }
        }
        None
    }

    // Port related methods
    pub fn add_port(&self, node_id: u32, port_id: u32, port: Port) {
        let private = imp::GraphView::from_instance(self);
        println!(
            "adding a port with port id {} to node id {}",
            port_id, node_id
        );
        if let Some(node) = private.nodes.borrow_mut().get_mut(&node_id) {
            node.add_port(port_id, port);
        } else {
            error!(
                "Node with id {} not found when trying to add port with id {} to graph",
                node_id, port_id
            );
        }
    }

    pub fn remove_port(&self, id: u32, node_id: u32) {
        let private = imp::GraphView::from_instance(self);
        let nodes = private.nodes.borrow();
        if let Some(node) = nodes.get(&node_id) {
            node.remove_port(id);
        }
    }

    pub fn port_is_linked(&self, port_id: u32) -> Option<(u32, u32, u32)> {
        let private = imp::GraphView::from_instance(self);
        for (key, link) in private.links.borrow().iter() {
            if link.port_from == port_id {
                return Some((*key, link.node_from, link.port_from));
            } else if link.port_to == port_id {
                return Some((*key, link.node_from, link.port_from));
            }
        }
        None
    }

    // Link related methods

    pub fn all_links(&self) -> Vec<NodeLink> {
        let private = imp::GraphView::from_instance(self);
        let links = private.links.borrow();
        let links_list: Vec<_> = links.iter().map(|(_, link)| link.clone()).collect();
        links_list
    }

    pub fn add_link(&self, link: NodeLink) {
        let private = imp::GraphView::from_instance(self);
        if !self.link_exists(&link) {
            private.links.borrow_mut().insert(link.id, link);
            self.queue_draw();
        }
    }

    pub fn set_link_state(&self, link_id: u32, active: bool) {
        let private = imp::GraphView::from_instance(self);
        if let Some(link) = private.links.borrow_mut().get_mut(&link_id) {
            link.active = active;
            self.queue_draw();
        } else {
            warn!("Link state changed on unknown link (id={})", link_id);
        }
    }

    pub fn remove_link(&self, id: u32) {
        let private = imp::GraphView::from_instance(self);
        let mut links = private.links.borrow_mut();
        links.remove(&id);

        self.queue_draw();
    }

    /// Get the position of the specified node inside the graphview.
    ///
    /// Returns `None` if the node is not in the graphview.
    pub(super) fn node_position(&self, node: &gtk::Widget) -> Option<(f32, f32)> {
        let layout_manager = self
            .layout_manager()
            .expect("Failed to get layout manager")
            .dynamic_cast::<gtk::FixedLayout>()
            .expect("Failed to cast to FixedLayout");

        let node = layout_manager
            .layout_child(node)?
            .dynamic_cast::<gtk::FixedLayoutChild>()
            .expect("Could not cast to FixedLayoutChild");
        let transform = node
            .transform()
            .expect("Failed to obtain transform from layout child");
        Some(transform.to_translate())
    }

    pub(super) fn move_node(&self, node: &gtk::Widget, x: f32, y: f32) {
        let layout_manager = self
            .layout_manager()
            .expect("Failed to get layout manager")
            .dynamic_cast::<gtk::FixedLayout>()
            .expect("Failed to cast to FixedLayout");

        let transform = gsk::Transform::new()
            // Nodes should not be able to be dragged out of the view, so we use `max(coordinate, 0.0)` to prevent that.
            .translate(&graphene::Point::new(f32::max(x, 0.0), f32::max(y, 0.0)))
            .unwrap();

        layout_manager
            .layout_child(node)
            .expect("Could not get layout child")
            .dynamic_cast::<gtk::FixedLayoutChild>()
            .expect("Could not cast to FixedLayoutChild")
            .set_transform(&transform);

        // FIXME: If links become proper widgets,
        // we don't need to redraw the full graph everytime.
        self.queue_draw();
    }

    pub(super) fn link_exists(&self, new_link: &NodeLink) -> bool {
        let private = imp::GraphView::from_instance(self);

        for link in private.links.borrow().values() {
            if (new_link.port_from == link.port_from && new_link.port_to == link.port_to)
                || (new_link.port_to == link.port_from && new_link.port_from == link.port_to)
            {
                println!("link already existing");
                return true;
            }
        }
        return false;
    }

    pub(super) fn ports_compatible(&self, to_port: &Port) -> bool {
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
                println!("Unable add the following link");
            }
            return res;
        }
        false
    }

    pub fn next_node_id(&self) -> u32 {
        let private = imp::GraphView::from_instance(self);
        private
            .current_node_id
            .set(private.current_node_id.get() + 1);
        private.current_node_id.get()
    }

    pub fn next_port_id(&self) -> u32 {
        let private = imp::GraphView::from_instance(self);
        private
            .current_port_id
            .set(private.current_port_id.get() + 1);
        private.current_port_id.get()
    }

    fn next_link_id(&self) -> u32 {
        let private = imp::GraphView::from_instance(self);
        private
            .current_link_id
            .set(private.current_link_id.get() + 1);
        private.current_link_id.get()
    }

    fn set_selected_port(&self, port: Option<&Port>) {
        let private = imp::GraphView::from_instance(self);
        *private.port_selected.borrow_mut() = port.cloned();
    }

    fn selected_port(&self) -> RefMut<Option<Port>> {
        let private = imp::GraphView::from_instance(self);
        private.port_selected.borrow_mut()
    }

    // Render graph methods
    pub fn render_gst(&self) -> String {
        let description = String::from("videotestsrc ! videoconvert ! autovideosink");
        description
    }

    pub fn render_xml(&self, filename: &str) -> anyhow::Result<(), Box<dyn error::Error>> {
        let mut file = File::create(filename).unwrap();
        let mut writer = EmitterConfig::new()
            .perform_indent(true)
            .create_writer(&mut file);

        writer.write(XMLWEvent::start_element("Graph"))?;

        //Get the nodes
        let nodes = self.all_nodes();
        for node in nodes {
            writer.write(
                XMLWEvent::start_element("Node")
                    .attr("name", &node.name())
                    .attr("id", &node.id().to_string())
                    .attr("type", &node.node_type().unwrap().to_string()),
            )?;
            for port in node.ports().values() {
                writer.write(
                    XMLWEvent::start_element("Port")
                        .attr("name", &port.name())
                        .attr("id", &port.id().to_string())
                        .attr("direction", &port.direction().to_string()),
                )?;
                writer.write(XMLWEvent::end_element())?;
            }
            writer.write(XMLWEvent::end_element())?;
        }
        //Get the link and write it.
        for link in self.all_links() {
            writer.write(
                XMLWEvent::start_element("Link")
                    .attr("id", &link.id.to_string())
                    .attr("node_from", &link.node_from.to_string())
                    .attr("node_to", &link.node_to.to_string())
                    .attr("port_from", &link.port_from.to_string())
                    .attr("port_to", &link.port_to.to_string())
                    .attr("active", &link.active.to_string()),
            )?;
            writer.write(XMLWEvent::end_element())?;
        }
        writer.write(XMLWEvent::end_element())?;
        Ok(())
    }

    pub fn load_xml(&self, filename: &str) -> anyhow::Result<(), Box<dyn error::Error>> {
        let file = File::open(filename).unwrap();
        let file = BufReader::new(file);

        let parser = EventReader::new(file);

        let mut current_node: Option<Node> = None;
        let mut current_port: Option<Port> = None;
        let mut current_link: Option<NodeLink> = None;
        for e in parser {
            match e {
                Ok(XMLREvent::StartElement {
                    ref name,
                    ref attributes,
                    ..
                }) => {
                    println!("{}", name);
                    let mut attrs = HashMap::new();
                    attributes.iter().for_each(|a| {
                        attrs.insert(a.name.to_string(), a.value.to_string());
                    });
                    match name.to_string().as_str() {
                        "Graph" => {
                            println!("New graph detected");
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

                            current_node = Some(Node::new(
                                id.parse::<u32>().unwrap(),
                                name,
                                NodeType::from_str(node_type.as_str()),
                            ));
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
                            current_port = Some(Port::new(
                                id.parse::<u32>().unwrap(),
                                name,
                                PortDirection::from_str(direction),
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
                            current_link = Some(NodeLink {
                                id: id.parse::<u32>().unwrap(),
                                node_from: node_from.parse::<u32>().unwrap(),
                                node_to: node_to.parse::<u32>().unwrap(),
                                port_from: port_from.parse::<u32>().unwrap(),
                                port_to: port_to.parse::<u32>().unwrap(),
                                active: active.parse::<bool>().unwrap(),
                            });
                        }
                        _ => println!("name unknown: {}", name),
                    }
                }
                Ok(XMLREvent::EndElement { name }) => {
                    println!("closing {}", name);
                    match name.to_string().as_str() {
                        "Graph" => {
                            println!("Graph ended");
                        }
                        "Node" => {
                            if let Some(node) = current_node {
                                let id = node.id();
                                self.add_node(id, node);
                            }
                            current_node = None;
                        }
                        "Port" => {
                            if let Some(port) = current_port {
                                let node = current_node.clone();
                                node.expect("No current node, error...")
                                    .add_port(port.id(), port);
                            }
                            current_port = None;
                        }
                        "Link" => {
                            if let Some(link) = current_link {
                                self.add_link(link);
                            }
                            current_link = None;
                        }
                        _ => println!("name unknown: {}", name),
                    }
                }
                Err(e) => {
                    println!("Error: {}", e);
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl Default for GraphView {
    fn default() -> Self {
        Self::new()
    }
}
