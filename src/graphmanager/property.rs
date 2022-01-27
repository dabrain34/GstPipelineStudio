// property.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
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
use std::cell::Ref;
use std::collections::HashMap;

pub trait PropertyExt {
    fn hidden_property(&self, name: &str) -> bool {
        name.starts_with('_')
    }

    /// Add a node property with a name and a value.
    ///
    fn add_property(&self, name: &str, value: &str);

    /// Update the properties.
    ///
    /// Update the PropertyExt properties.
    ///
    fn update_properties(&self, new_properties: &HashMap<String, String>) {
        for (key, value) in new_properties {
            self.add_property(key, value);
        }
    }

    /// Retrieves properties.
    ///
    fn properties(&self) -> Ref<HashMap<String, String>>;

    /// Retrieves property with the name.
    ///
    /// Retrieves node property with the name.
    ///
    fn property(&self, name: &str) -> Option<String> {
        if let Some(property) = self.properties().get(name) {
            return Some(property.clone());
        }
        None
    }
}
