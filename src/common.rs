// common.rs
//
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

use anyhow::Result;
use gstreamer as gst;

pub const APPLICATION_NAME: &str = "org.freedesktop.gst-pipeline-studio";

pub fn init() -> Result<()> {
    unsafe {
        x11::xlib::XInitThreads();
    }
    gst::init()?;
    gtk::init()?;
    Ok(())
}
