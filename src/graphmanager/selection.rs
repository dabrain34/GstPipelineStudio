// selection.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GraphManager
//
// SPDX-License-Identifier: GPL-3.0-only

pub trait SelectionExt {
    /// Toggle selected
    ///
    fn toggle_selected(&self);

    /// Set selection to selected state
    ///
    fn set_selected(&self, selected: bool);

    /// Retrieve selection state
    ///
    fn selected(&self) -> bool;
}
