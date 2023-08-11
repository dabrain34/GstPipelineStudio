// link.rs
//
// Copyright 2021 Tom A. Wagner <tom.a.wagner@protonmail.com>
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GraphManager
//
// SPDX-License-Identifier: GPL-3.0-only

use super::SelectionExt;
use std::cell::{Cell, RefCell};

#[derive(Debug, Clone)]
pub struct Link {
    pub id: u32,
    pub node_from: u32,
    pub node_to: u32,
    pub port_from: u32,
    pub port_to: u32,
    pub active: Cell<bool>,
    pub selected: Cell<bool>,
    pub thickness: u32,
    pub name: RefCell<String>,
}

impl Link {
    pub fn name(&self) -> String {
        self.name.borrow().clone()
    }

    pub fn set_name(&self, name: &str) {
        self.name.replace(name.to_string());
    }
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn active(&self) -> bool {
        self.active.get()
    }
    pub fn set_active(&self, active: bool) {
        self.active.set(active)
    }
}

pub trait LinkExt {
    /// Create a new link
    ///
    fn new(id: u32, node_from: u32, node_to: u32, port_from: u32, port_to: u32) -> Self;
}

impl LinkExt for Link {
    fn new(id: u32, node_from: u32, node_to: u32, port_from: u32, port_to: u32) -> Self {
        Self {
            id,
            node_from,
            node_to,
            port_from,
            port_to,
            active: Cell::new(true),
            selected: Cell::new(false),
            thickness: 4,
            name: RefCell::new("".to_string()),
        }
    }
}
impl SelectionExt for Link {
    fn toggle_selected(&self) {
        self.set_selected(!self.selected.get());
    }

    fn set_selected(&self, selected: bool) {
        self.selected.set(selected);
    }

    fn selected(&self) -> bool {
        self.selected.get()
    }
}
