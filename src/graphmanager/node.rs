// node.rs
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
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use log::trace;

use super::Port;
use super::PortDirection;

use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
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
    use gtk::Orientation;
    use once_cell::unsync::OnceCell;
    pub struct Node {
        pub(super) layoutbox: gtk::Box,
        pub(super) inputs: gtk::Box,
        pub(super) outputs: gtk::Box,
        pub(super) name: gtk::Label,
        pub(super) description: gtk::Label,
        pub(super) id: OnceCell<u32>,
        pub(super) node_type: OnceCell<NodeType>,
        pub(super) ports: RefCell<HashMap<u32, Port>>,
        pub(super) num_ports_in: Cell<i32>,
        pub(super) num_ports_out: Cell<i32>,
        // Properties are differnet from GObject properties
        pub(super) properties: RefCell<HashMap<String, String>>,
        pub(super) selected: Cell<bool>,
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
            let layoutbox = gtk::Box::new(Orientation::Vertical, 6);
            let name_desc = gtk::Box::new(Orientation::Vertical, 6);
            layoutbox.append(&name_desc);
            let ports = gtk::Box::builder()
                .orientation(Orientation::Horizontal)
                .halign(gtk::Align::Start)
                .spacing(10)
                .margin_bottom(10)
                .margin_top(10)
                .build();

            layoutbox.append(&ports);
            let inputs = gtk::Box::builder()
                .orientation(Orientation::Vertical)
                .halign(gtk::Align::Start)
                .spacing(10)
                .build();

            ports.append(&inputs);
            let center = gtk::Box::builder()
                .orientation(Orientation::Vertical)
                .halign(gtk::Align::Center)
                .hexpand(true)
                .margin_start(20)
                .margin_end(20)
                .build();
            ports.append(&center);
            let outputs = gtk::Box::builder()
                .orientation(Orientation::Vertical)
                .halign(gtk::Align::End)
                .spacing(10)
                .build();
            ports.append(&outputs);

            let name = gtk::Label::new(None);
            name_desc.append(&name);

            let description = gtk::Label::new(None);
            name_desc.append(&description);

            // Display a grab cursor when the mouse is over the name so the user knows the node can be dragged.
            name.set_cursor(gtk::gdk::Cursor::from_name("grab", None).as_ref());

            Self {
                layoutbox,
                inputs,
                outputs,
                name,
                description,
                id: OnceCell::new(),
                node_type: OnceCell::new(),
                ports: RefCell::new(HashMap::new()),
                num_ports_in: Cell::new(0),
                num_ports_out: Cell::new(0),
                properties: RefCell::new(HashMap::new()),
                selected: Cell::new(false),
            }
        }
    }

    impl ObjectImpl for Node {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            self.layoutbox.set_parent(obj);
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.layoutbox.unparent();
        }
    }

    impl WidgetImpl for Node {}
}

glib::wrapper! {
    pub struct Node(ObjectSubclass<imp::Node>)
        @extends gtk::Widget, gtk::Box;
}

impl Node {
    pub fn new(id: u32, name: &str, node_type: NodeType) -> Self {
        let res: Self = glib::Object::new(&[]).expect("Failed to create Node");
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

    fn set_name(&self, name: &str) {
        let self_ = imp::Node::from_instance(self);
        self_.name.set_text(name);
    }

    fn set_description(&self, description: &str) {
        let self_ = imp::Node::from_instance(self);
        self_.description.set_text(description);
        trace!("Node description is {}", description);
    }
    pub fn hidden_property(&self, name: &str) -> bool {
        name.starts_with('_')
    }

    fn update_description(&self) {
        let self_ = imp::Node::from_instance(self);
        let mut description = String::from("");
        for (name, value) in self_.properties.borrow().iter() {
            if !self.hidden_property(name) {
                description.push_str(&format!("{}:{}", name, value));
                description.push('\n');
            }
        }
        self.set_description(&description);
    }

    pub fn add_port(&mut self, id: u32, name: &str, direction: PortDirection) {
        let private = imp::Node::from_instance(self);
        let port = Port::new(id, name, direction);
        match port.direction() {
            PortDirection::Input => {
                private.inputs.append(&port);
                private.num_ports_in.set(private.num_ports_in.get() + 1);
            }
            PortDirection::Output => {
                private.outputs.append(&port);
                private.num_ports_out.set(private.num_ports_out.get() + 1);
            }
            _ => panic!("Port without direction"),
        }

        private.ports.borrow_mut().insert(id, port);
    }

    pub fn ports(&self) -> Ref<HashMap<u32, Port>> {
        let private = imp::Node::from_instance(self);
        private.ports.borrow()
    }

    pub fn all_ports(&self, direction: PortDirection) -> Vec<Port> {
        let ports_list: Vec<_> = self
            .ports()
            .iter()
            .filter(|(_, port)| port.direction() == direction || direction == PortDirection::All)
            .map(|(_, port)| port.clone())
            .collect();
        ports_list
    }

    pub fn port(&self, id: &u32) -> Option<super::port::Port> {
        let private = imp::Node::from_instance(self);
        private.ports.borrow().get(id).cloned()
    }

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

    pub fn id(&self) -> u32 {
        let private = imp::Node::from_instance(self);
        private.id.get().copied().expect("Node id is not set")
    }

    pub fn name(&self) -> String {
        let private = imp::Node::from_instance(self);
        private.name.text().to_string()
    }

    pub fn unique_name(&self) -> String {
        let private = imp::Node::from_instance(self);
        let mut unique_name = private.name.text().to_string();
        unique_name.push_str(&self.id().to_string());
        unique_name
    }

    pub fn node_type(&self) -> Option<&NodeType> {
        let private = imp::Node::from_instance(self);
        private.node_type.get()
    }

    pub fn add_property(&self, name: String, value: String) {
        let private = imp::Node::from_instance(self);
        trace!("property name={} updated with value={}", name, value);
        private.properties.borrow_mut().insert(name, value);
        self.update_description();
    }

    pub fn update_properties(&self, new_node_properties: &HashMap<String, String>) {
        for (key, value) in new_node_properties {
            self.add_property(key.clone(), value.clone());
        }
    }

    pub fn properties(&self) -> Ref<HashMap<String, String>> {
        let private = imp::Node::from_instance(self);
        private.properties.borrow()
    }

    pub fn property(&self, name: &str) -> Option<String> {
        let private = imp::Node::from_instance(self);
        if let Some(property) = private.properties.borrow().get(name) {
            return Some(property.clone());
        }
        None
    }

    pub fn toggle_selected(&self) {
        self.set_selected(!self.selected());
    }

    pub fn set_selected(&self, selected: bool) {
        let private = imp::Node::from_instance(self);
        private.selected.set(selected);
        if selected {
            self.add_css_class("node-selected");
        } else {
            self.remove_css_class("node-selected");
        }
    }

    pub fn selected(&self) -> bool {
        let private = imp::Node::from_instance(self);
        private.selected.get()
    }

    pub fn unselect_all_ports(&self) {
        let private = imp::Node::from_instance(self);
        for port in private.ports.borrow_mut().values() {
            port.set_selected(false);
        }
    }
}
