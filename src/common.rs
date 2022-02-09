// common.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;

pub fn init() -> Result<()> {
    unsafe {
        x11::xlib::XInitThreads();
    }
    gst::init()?;
    gtk::init()?;
    Ok(())
}
