// port.rs
//
// Copyright 2021 Tom A. Wagner <tom.a.wagner@protonmail.com>
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GraphManager
//
// SPDX-License-Identifier: GPL-3.0-only

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, graphene};
use log::trace;
use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::fmt;

use super::{PropertyExt, SelectionExt};

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Copy)]
pub enum PortDirection {
    Input,
    Output,
    All,
    Unknown,
}

impl fmt::Display for PortDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl PortDirection {
    pub fn from_str(port_direction_name: &str) -> PortDirection {
        match port_direction_name {
            "Input" => PortDirection::Input,
            "Output" => PortDirection::Output,
            "All" => PortDirection::All,
            _ => PortDirection::Unknown,
        }
    }
}

/// Port's presence
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Copy)]
pub enum PortPresence {
    /// Can not be removed from his parent independently
    Always,
    /// Can be removed from a node
    Sometimes,
    Unknown,
}

impl fmt::Display for PortPresence {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl PortPresence {
    pub fn from_str(port_direction_name: &str) -> PortPresence {
        match port_direction_name {
            "Always" => PortPresence::Always,
            "Sometimes" => PortPresence::Sometimes,
            _ => PortPresence::Unknown,
        }
    }
}

mod imp {
    use super::*;
    use once_cell::unsync::OnceCell;

    /// Graphical representation of a port.
    #[derive(Default)]
    pub struct Port {
        pub(super) id: OnceCell<u32>,
        pub(super) name: RefCell<String>,
        pub(super) port_direction: OnceCell<super::PortDirection>,
        pub(super) selected: Cell<bool>,
        pub(super) presence: OnceCell<super::PortPresence>,
        pub(super) properties: RefCell<HashMap<String, String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Port {
        const NAME: &'static str = "Port";
        type Type = super::Port;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("port");
        }
    }

    impl ObjectImpl for Port {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();
            obj.set_halign(gtk::Align::Center);
            obj.set_valign(gtk::Align::Center);
        }
    }

    impl WidgetImpl for Port {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            (Self::HANDLE_SIZE, Self::HANDLE_SIZE, -1, -1)
        }
    }

    impl Port {
        pub const HANDLE_SIZE: i32 = 10;
    }
}

glib::wrapper! {
    pub struct Port(ObjectSubclass<imp::Port>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Port {
    /// Create a new port
    pub fn new(id: u32, name: &str, direction: PortDirection, presence: PortPresence) -> Self {
        let port: Self = glib::Object::new();
        let private = imp::Port::from_obj(&port);

        private.id.set(id).expect("Port id already set");
        private.selected.set(false);
        private
            .port_direction
            .set(direction)
            .expect("Port direction already set");

        // Add CSS classes
        if direction == PortDirection::Input {
            port.add_css_class("port-in");
        } else {
            port.add_css_class("port-out");
        }

        private
            .presence
            .set(presence)
            .expect("Port presence already set");
        if presence == PortPresence::Always {
            port.add_css_class("port-always");
        } else {
            port.add_css_class("port-sometimes");
        }

        // Store the name internally - shown in tooltip only
        *private.name.borrow_mut() = name.to_string();

        port
    }

    /// Retrieves the port id
    pub fn id(&self) -> u32 {
        let private = imp::Port::from_obj(self);
        private.id.get().copied().expect("Port id is not set")
    }

    /// Retrieves the port name
    pub fn name(&self) -> String {
        let private = imp::Port::from_obj(self);
        private.name.borrow().clone()
    }

    /// Set the port name
    pub fn set_name(&self, name: &str) {
        let private = imp::Port::from_obj(self);
        *private.name.borrow_mut() = name.to_string();
    }

    /// Retrieves the port direction
    pub fn direction(&self) -> PortDirection {
        let private = imp::Port::from_obj(self);
        *private
            .port_direction
            .get()
            .expect("Port direction is not set")
    }

    /// Retrieves the port presence
    pub fn presence(&self) -> PortPresence {
        let private = imp::Port::from_obj(self);
        *private.presence.get().expect("Port presence is not set")
    }

    /// Get link anchor point for drawing connections
    pub fn get_link_anchor(&self) -> graphene::Point {
        graphene::Point::new(
            imp::Port::HANDLE_SIZE as f32 / 2.0,
            imp::Port::HANDLE_SIZE as f32 / 2.0,
        )
    }
}

impl SelectionExt for Port {
    fn toggle_selected(&self) {
        self.set_selected(!self.selected());
    }

    fn set_selected(&self, selected: bool) {
        let private = imp::Port::from_obj(self);
        private.selected.set(selected);
        if selected {
            self.add_css_class("port-selected");
        } else {
            self.remove_css_class("port-selected");
        }
    }

    fn selected(&self) -> bool {
        let private = imp::Port::from_obj(self);
        private.selected.get()
    }
}

impl PropertyExt for Port {
    fn add_property(&self, name: &str, value: &str) {
        let private = imp::Port::from_obj(self);
        trace!("property name={} updated with value={}", name, value);
        private
            .properties
            .borrow_mut()
            .insert(name.to_string(), value.to_string());
    }

    fn remove_property(&self, name: &str) {
        let private = imp::Port::from_obj(self);
        trace!("property name={} removed", name);
        private.properties.borrow_mut().remove(name);
    }

    fn properties(&self) -> Ref<'_, HashMap<String, String>> {
        let private = imp::Port::from_obj(self);
        private.properties.borrow()
    }
}
