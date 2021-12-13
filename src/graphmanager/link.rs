// link.rs
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

use std::cell::Cell;

#[derive(Debug, Clone)]
pub struct Link {
    pub id: u32,
    pub node_from: u32,
    pub node_to: u32,
    pub port_from: u32,
    pub port_to: u32,
    pub active: bool,
    pub selected: Cell<bool>,
    pub thickness: u32,
}

impl Link {
    pub fn new(
        id: u32,
        node_from: u32,
        node_to: u32,
        port_from: u32,
        port_to: u32,
        active: bool,
        selected: bool,
    ) -> Self {
        Self {
            id,
            node_from,
            node_to,
            port_from,
            port_to,
            active,
            selected: Cell::new(selected),
            thickness: 4,
        }
    }

    pub fn toggle_selected(&self) {
        self.set_selected(!self.selected.get());
    }

    pub fn set_selected(&self, selected: bool) {
        self.selected.set(selected);
    }

    pub fn selected(&self) -> bool {
        self.selected.get()
    }
}
