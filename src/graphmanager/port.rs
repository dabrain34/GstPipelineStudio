// port.rs
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
use gtk::{
    glib::{self},
    prelude::*,
    subclass::prelude::*,
};
use log::info;
use std::cell::RefCell;
use std::cell::{Cell, Ref};
use std::collections::HashMap;
use std::fmt;

use super::{PropertyExt, SelectionExt};

#[derive(Debug, Clone, PartialOrd, PartialEq, Copy)]
pub enum PortDirection {
    Input,
    Output,
    All,
    Unknown,
}

impl fmt::Display for PortDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
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
#[derive(Debug, Clone, PartialEq, PartialOrd, Copy)]
pub enum PortPresence {
    /// Can not be removed from his parent independantly
    Always,
    /// Can be removed from a node
    Sometimes,
    Unknown,
}

impl fmt::Display for PortPresence {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
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
    #[derive(Default, Clone)]
    pub struct Port {
        pub(super) label: gtk::Label,
        pub(super) id: OnceCell<u32>,
        pub(super) direction: OnceCell<PortDirection>,
        pub(super) selected: Cell<bool>,
        pub(super) presence: OnceCell<PortPresence>,
        pub(super) properties: RefCell<HashMap<String, String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Port {
        const NAME: &'static str = "Port";
        type Type = super::Port;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();

            // Make it look like a GTK button.
            klass.set_css_name("button");
        }

        fn new() -> Self {
            let label = gtk::Label::new(None);
            Self {
                label,
                id: OnceCell::new(),
                direction: OnceCell::new(),
                selected: Cell::new(false),
                presence: OnceCell::new(),
                properties: RefCell::new(HashMap::new()),
            }
        }
    }

    impl ObjectImpl for Port {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            self.label.set_parent(obj);
        }
        fn dispose(&self, _obj: &Self::Type) {
            self.label.unparent()
        }
    }
    impl WidgetImpl for Port {}
}

glib::wrapper! {
    pub struct Port(ObjectSubclass<imp::Port>)
        @extends gtk::Widget, gtk::Box;
}

impl Port {
    /// Create a new port
    ///
    pub fn new(id: u32, name: &str, direction: PortDirection, presence: PortPresence) -> Self {
        // Create the widget and initialize needed fields
        let port: Self = glib::Object::new(&[]).expect("Failed to create Port");
        port.add_css_class("port");
        let private = imp::Port::from_instance(&port);
        private.id.set(id).expect("Port id already set");
        private.selected.set(false);
        private
            .direction
            .set(direction)
            .expect("Port direction already set");
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
        private.label.set_text(name);

        port
    }

    /// Retrieves the port id
    ///
    pub fn id(&self) -> u32 {
        let private = imp::Port::from_instance(self);
        private.id.get().copied().expect("Port id is not set")
    }

    /// Retrieves the port name
    ///
    pub fn name(&self) -> String {
        let private = imp::Port::from_instance(self);
        private.label.text().to_string()
    }

    /// Retrieves the port direction
    ///
    pub fn direction(&self) -> PortDirection {
        let private = imp::Port::from_instance(self);
        *private.direction.get().expect("Port direction is not set")
    }

    /// Retrieves the port presence
    ///
    pub fn presence(&self) -> PortPresence {
        let private = imp::Port::from_instance(self);
        *private.presence.get().expect("Port presence is not set")
    }
}

impl SelectionExt for Port {
    fn toggle_selected(&self) {
        self.set_selected(!self.selected());
    }

    fn set_selected(&self, selected: bool) {
        let private = imp::Port::from_instance(self);
        private.selected.set(selected);
        if selected {
            self.add_css_class("port-selected");
        } else {
            self.remove_css_class("port-selected");
        }
    }

    fn selected(&self) -> bool {
        let private = imp::Port::from_instance(self);
        private.selected.get()
    }
}

impl PropertyExt for Port {
    /// Add a node property with a name and a value.
    ///
    fn add_property(&self, name: &str, value: &str) {
        let private = imp::Port::from_instance(self);
        info!("property name={} updated with value={}", name, value);
        private
            .properties
            .borrow_mut()
            .insert(name.to_string(), value.to_string());
    }

    /// Retrieves node properties.
    ///
    fn properties(&self) -> Ref<HashMap<String, String>> {
        let private = imp::Port::from_instance(self);
        private.properties.borrow()
    }
}
