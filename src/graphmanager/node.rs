// node.rs
//
// Copyright 2021 Tom A. Wagner <tom.a.wagner@protonmail.com>
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GraphManager
//
// SPDX-License-Identifier: GPL-3.0-only

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use log::trace;

use super::{Port, PortDirection, PortPresence, PropertyExt, SelectionExt};

use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;

use std::fmt;
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
    Source,
    Transform,
    Sink,
    All,
    Unknown,
}

impl fmt::Display for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl NodeType {
    pub fn from_str(node_type_name: &str) -> NodeType {
        match node_type_name {
            "Source" => NodeType::Source,
            "Transform" => NodeType::Transform,
            "Sink" => NodeType::Sink,
            "All" => NodeType::All,
            _ => NodeType::Unknown,
        }
    }
}

mod imp {
    use super::*;
    use once_cell::unsync::OnceCell;
    pub struct Node {
        pub(super) layoutgrid: gtk::Grid,
        pub(super) name: gtk::Label,
        pub(super) description: gtk::Label,
        pub(super) id: OnceCell<u32>,
        pub(super) node_type: OnceCell<NodeType>,
        pub(super) ports: RefCell<HashMap<u32, Port>>,
        pub(super) num_ports_in: Cell<i32>,
        pub(super) num_ports_out: Cell<i32>,
        // Properties are different from GObject properties
        pub(super) properties: RefCell<HashMap<String, String>>,
        pub(super) selected: Cell<bool>,
        pub(super) light: Cell<bool>,
        pub(super) position: Cell<(f32, f32)>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Node {
        const NAME: &'static str = "Node";
        type Type = super::Node;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
            klass.set_css_name("button");
        }

        fn new() -> Self {
            let layoutgrid = gtk::Grid::builder()
                .margin_start(6)
                .margin_end(6)
                .margin_top(6)
                .margin_bottom(6)
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Center)
                .row_spacing(6)
                .column_spacing(6)
                .build();

            let name = gtk::Label::new(None);
            layoutgrid.attach(&name, 1, 0, 1, 1);

            let description = gtk::Label::new(None);
            layoutgrid.attach(&description, 1, 1, 1, 1);

            // Display a grab cursor when the mouse is over the name so the user knows the node can be dragged.
            name.set_cursor(gtk::gdk::Cursor::from_name("grab", None).as_ref());

            Self {
                layoutgrid,
                name,
                description,
                id: OnceCell::new(),
                node_type: OnceCell::new(),
                ports: RefCell::new(HashMap::new()),
                num_ports_in: Cell::new(0),
                num_ports_out: Cell::new(0),
                properties: RefCell::new(HashMap::new()),
                selected: Cell::new(false),
                light: Cell::new(false),
                position: Cell::new((0.0, 0.0)),
            }
        }
    }

    impl ObjectImpl for Node {
        fn constructed(&self) {
            let obj = self.obj();
            self.parent_constructed();
            self.layoutgrid.set_parent(&*obj);
        }

        fn dispose(&self) {
            self.layoutgrid.unparent();
        }
    }

    impl WidgetImpl for Node {}
}

glib::wrapper! {
    pub struct Node(ObjectSubclass<imp::Node>)
        @extends gtk::Widget, gtk::Box;
}

impl Node {
    /// Create a new node
    ///
    pub fn new(id: u32, name: &str, node_type: NodeType) -> Self {
        let res = glib::Object::new::<Self>(&[]);
        let private = imp::Node::from_instance(&res);
        private.id.set(id).expect("Node id is already set");
        res.set_name(name);
        res.add_css_class("node");
        private
            .node_type
            .set(node_type)
            .expect("Node type is already set");
        res
    }

    /// Add a new port to the node
    ///
    pub fn add_port(&mut self, port: Port) {
        let private = imp::Node::from_instance(self);
        match port.direction() {
            PortDirection::Input => {
                private
                    .layoutgrid
                    .attach(&port, 0, private.num_ports_in.get(), 1, 1);
                private.num_ports_in.set(private.num_ports_in.get() + 1);
            }
            PortDirection::Output => {
                private
                    .layoutgrid
                    .attach(&port, 2, private.num_ports_out.get(), 1, 1);
                private.num_ports_out.set(private.num_ports_out.get() + 1);
            }
            _ => panic!("Port without direction"),
        }
        private.ports.borrow_mut().insert(port.id(), port);
    }

    /// Retrieves all ports as an hashmap
    ///
    pub fn ports(&self) -> Ref<HashMap<u32, Port>> {
        let private = imp::Node::from_instance(self);
        private.ports.borrow()
    }

    /// Retrieves all ports with given direction
    ///
    pub fn all_ports(&self, direction: PortDirection) -> Vec<Port> {
        let ports_list: Vec<_> = self
            .ports()
            .iter()
            .filter(|(_, port)| port.direction() == direction || direction == PortDirection::All)
            .map(|(_, port)| port.clone())
            .collect();
        ports_list
    }

    /// Retrieves the port with id
    ///
    pub fn port(&self, id: u32) -> Option<super::port::Port> {
        let private = imp::Node::from_instance(self);
        private.ports.borrow().get(&id).cloned()
    }

    /// Check if we can remove a port dependending on PortPrensence attribute
    ///
    pub fn can_remove_port(&self, id: u32) -> bool {
        let private = imp::Node::from_instance(self);
        if let Some(port) = private.ports.borrow().get(&id) {
            if port.presence() != PortPresence::Always {
                return true;
            }
        }
        false
    }

    /// Removes a port id from the given node
    ///
    pub fn remove_port(&self, id: u32) {
        let private = imp::Node::from_instance(self);
        if let Some(port) = private.ports.borrow_mut().remove(&id) {
            match port.direction() {
                PortDirection::Input => private.num_ports_in.set(private.num_ports_in.get() - 1),
                PortDirection::Output => private.num_ports_in.set(private.num_ports_out.get() - 1),
                _ => panic!("Port without direction"),
            }
            port.unparent();
        }
    }

    /// Retrieves the node id
    ///
    pub fn id(&self) -> u32 {
        let private = imp::Node::from_instance(self);
        private.id.get().copied().expect("Node id is not set")
    }

    /// Retrieves the node name
    ///
    pub fn name(&self) -> String {
        let private = imp::Node::from_instance(self);
        private.name.text().to_string()
    }

    /// Retrieves the unique name composed with the node name and its id
    ///
    pub fn unique_name(&self) -> String {
        let private = imp::Node::from_instance(self);
        let mut unique_name = private.name.text().to_string();
        unique_name.push_str(&self.id().to_string());
        unique_name
    }

    /// Retrieves the NodeType
    ///
    pub fn node_type(&self) -> Option<&NodeType> {
        let private = imp::Node::from_instance(self);
        private.node_type.get()
    }

    /// Unselect all the ports of the given node.
    ///
    pub fn unselect_all_ports(&self) {
        let private = imp::Node::from_instance(self);
        for port in private.ports.borrow_mut().values() {
            port.set_selected(false);
        }
    }

    /// Set coordinates for the drawn node.
    ///
    pub fn set_position(&self, x: f32, y: f32) {
        imp::Node::from_instance(self).position.set((x, y));
    }

    /// Get coordinates for the drawn node.
    ///
    /// # Returns
    /// `(x, y)`
    pub fn position(&self) -> (f32, f32) {
        imp::Node::from_instance(self).position.get()
    }

    pub fn set_light(&self, light: bool) {
        let self_ = imp::Node::from_instance(self);
        self_.light.set(light);
        if light {
            self.add_css_class("node-light");
        } else {
            self.remove_css_class("node-light");
        }
    }

    pub fn light(&self) -> bool {
        let self_ = imp::Node::from_instance(self);
        self_.light.get()
    }

    //Private

    fn set_name(&self, name: &str) {
        let self_ = imp::Node::from_instance(self);
        self_.name.set_text(name);
    }

    fn set_description(&self, description: &str) {
        let self_ = imp::Node::from_instance(self);
        self_.description.set_text(description);
        trace!("Node description is {}", description);
    }

    fn update_description(&self) {
        let self_ = imp::Node::from_instance(self);
        let mut description = String::from("");
        for (name, value) in self_.properties.borrow().iter() {
            if !self.hidden_property(name) {
                let _ = write!(description, "{}:{}", name, value);
                description.push('\n');
            }
        }
        self.set_description(&description);
    }
}

impl SelectionExt for Node {
    fn toggle_selected(&self) {
        self.set_selected(!self.selected());
    }

    fn set_selected(&self, selected: bool) {
        let private = imp::Node::from_instance(self);
        private.selected.set(selected);
        if selected {
            self.add_css_class("node-selected");
        } else {
            self.remove_css_class("node-selected");
        }
    }

    fn selected(&self) -> bool {
        let private = imp::Node::from_instance(self);
        private.selected.get()
    }
}

impl PropertyExt for Node {
    /// Add a node property with a name and a value.
    ///
    fn add_property(&self, name: &str, value: &str) {
        let private = imp::Node::from_instance(self);
        trace!("property name={} updated with value={}", name, value);
        private
            .properties
            .borrow_mut()
            .insert(name.to_string(), value.to_string());
        self.update_description();
    }

    /// Remove a port property with a name.
    ///
    fn remove_property(&self, name: &str) {
        let private = imp::Node::from_instance(self);
        trace!("property name={} removed", name);
        private.properties.borrow_mut().remove(name);
        self.update_description();
    }

    /// Retrieves node properties.
    ///
    fn properties(&self) -> Ref<HashMap<String, String>> {
        let private = imp::Node::from_instance(self);
        private.properties.borrow()
    }
}
