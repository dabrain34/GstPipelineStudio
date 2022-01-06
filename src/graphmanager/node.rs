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
    use once_cell::unsync::OnceCell;
    pub struct Node {
        pub(super) grid: gtk::Grid,
        pub(super) label: gtk::Label,
        pub(super) id: OnceCell<u32>,
        pub(super) node_type: OnceCell<NodeType>,
        pub(super) ports: RefCell<HashMap<u32, Port>>,
        pub(super) num_ports_in: Cell<i32>,
        pub(super) num_ports_out: Cell<i32>,
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
            let grid = gtk::Grid::new();
            let label = gtk::Label::new(None);

            grid.attach(&label, 0, 0, 2, 1);

            // Display a grab cursor when the mouse is over the label so the user knows the node can be dragged.
            label.set_cursor(gtk::gdk::Cursor::from_name("grab", None).as_ref());

            Self {
                grid,
                label,
                id: OnceCell::new(),
                node_type: OnceCell::new(),
                ports: RefCell::new(HashMap::new()),
                num_ports_in: Cell::new(0),
                num_ports_out: Cell::new(0),
            }
        }
    }

    impl ObjectImpl for Node {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            self.grid.set_parent(obj);
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.grid.unparent();
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
        private.id.set(id).expect("Node id already set");
        res.set_name(name);
        private
            .node_type
            .set(node_type)
            .expect("Node type is already set");
        res
    }

    fn set_name(&self, name: &str) {
        let self_ = imp::Node::from_instance(self);
        self_.label.set_text(name);
        println!("{}", name);
    }

    pub fn add_port(&mut self, id: u32, port: super::port::Port) {
        let private = imp::Node::from_instance(self);

        match port.direction() {
            PortDirection::Input => {
                private
                    .grid
                    .attach(&port, 0, private.num_ports_in.get() + 1, 1, 1);
                private.num_ports_in.set(private.num_ports_in.get() + 1);
            }
            PortDirection::Output => {
                private
                    .grid
                    .attach(&port, 1, private.num_ports_out.get() + 1, 1, 1);
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
        private.label.text().to_string()
    }

    pub fn unique_name(&self) -> String {
        let private = imp::Node::from_instance(self);
        let mut unique_name = private.label.text().to_string();
        unique_name.push_str(&self.id().to_string());
        unique_name
    }

    pub fn node_type(&self) -> Option<&NodeType> {
        let private = imp::Node::from_instance(self);
        private.node_type.get()
    }
}
