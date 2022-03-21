// main.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only
use crate::app::GPSAppWeak;
use crate::gps as GPS;
use crate::graphmanager as GM;
use std::cell::Ref;
use std::cell::RefCell;

#[derive(Debug)]
pub struct GPSSession {
    pub graphview: RefCell<GM::GraphView>,
    pub player: RefCell<GPS::Player>,
}

impl GPSSession {
    pub fn new() -> Self {
        GPSSession {
            graphview: RefCell::new(GM::GraphView::new()),
            player: RefCell::new(
                GPS::Player::new().expect("Unable to initialize GStreamer subsystem"),
            ),
        }
    }

    pub fn graphview(&self) -> Ref<GM::GraphView> {
        self.graphview.borrow()
    }

    pub fn set_graphview_id(&self, id: u32) {
        self.graphview.borrow().set_id(id);
    }

    pub fn player(&self) -> Ref<GPS::Player> {
        self.player.borrow()
    }

    pub fn set_player(&self, app: GPSAppWeak) {
        self.player.borrow().set_app(app);
    }
}
