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
    glib::{self, subclass::Signal},
    prelude::*,
    subclass::prelude::*,
};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum PortDirection {
    Input,
    Output,
    All,
    Unknown,
}

impl fmt::Display for PortDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
}

impl PortDirection {
    pub fn from_str(port_direction_name: &str) -> PortDirection {
        match port_direction_name {
            "Input" => PortDirection::Input,
            "Output" => PortDirection::Output,
            "All" => PortDirection::Output,
            _ => PortDirection::Unknown,
        }
    }
}

mod imp {
    use super::*;
    use once_cell::{sync::Lazy, unsync::OnceCell};

    /// Graphical representation of a pipewire port.
    #[derive(Default, Clone)]
    pub struct Port {
        pub(super) label: OnceCell<gtk::Label>,
        pub(super) id: OnceCell<u32>,
        pub(super) direction: OnceCell<PortDirection>,
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
    }

    impl ObjectImpl for Port {
        fn dispose(&self, _obj: &Self::Type) {
            if let Some(label) = self.label.get() {
                label.unparent()
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder(
                    "port-toggled",
                    // Provide id of output port and input port to signal handler.
                    &[<u32>::static_type().into(), <u32>::static_type().into()],
                    // signal handler sends back nothing.
                    <()>::static_type().into(),
                )
                .build()]
            });

            SIGNALS.as_ref()
        }
    }
    impl WidgetImpl for Port {}
}

glib::wrapper! {
    pub struct Port(ObjectSubclass<imp::Port>)
        @extends gtk::Widget, gtk::Box;
}

impl Port {
    pub fn new(id: u32, name: &str, direction: PortDirection) -> Self {
        // Create the widget and initialize needed fields
        let res: Self = glib::Object::new(&[]).expect("Failed to create Port");

        let private = imp::Port::from_instance(&res);
        private.id.set(id).expect("Port id already set");
        private
            .direction
            .set(direction)
            .expect("Port direction already set");

        let label = gtk::Label::new(Some(name));
        label.set_parent(&res);
        private
            .label
            .set(label)
            .expect("Port label was already set");

        // Display a grab cursor when the mouse is over the port so the user knows it can be dragged to another port.
        res.set_cursor(gtk::gdk::Cursor::from_name("grab", None).as_ref());

        res
    }

    pub fn id(&self) -> u32 {
        let private = imp::Port::from_instance(self);
        private.id.get().copied().expect("Port id is not set")
    }

    pub fn direction(&self) -> &PortDirection {
        let private = imp::Port::from_instance(self);
        private.direction.get().expect("Port direction is not set")
    }

    pub fn name(&self) -> String {
        let private = imp::Port::from_instance(self);
        private
            .direction
            .get()
            .expect("direction is not set")
            .to_string()
    }
}
