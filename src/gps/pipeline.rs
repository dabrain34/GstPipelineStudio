// pipeline.rs
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

use crate::app::{AppState, GPSApp, GPSAppWeak};
use crate::graphmanager::{GraphView, Node, NodeType, PortDirection, PropertyExt};

use crate::gps::ElementInfo;
use crate::logger;
use crate::GPS_INFO;

use gst::glib;
use gst::prelude::*;
use gtk::gdk;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::ops;
use std::rc::{Rc, Weak};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PipelineState {
    Playing,
    Paused,
    Stopped,
    Error,
}

impl fmt::Display for PipelineState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
pub struct Pipeline(Rc<PipelineInner>);

// Deref into the contained struct to make usage a bit more ergonomic
impl ops::Deref for Pipeline {
    type Target = PipelineInner;

    fn deref(&self) -> &PipelineInner {
        &*self.0
    }
}

#[derive(Debug, Clone)]
pub struct PipelineWeak(Weak<PipelineInner>);

impl PipelineWeak {
    pub fn upgrade(&self) -> Option<Pipeline> {
        self.0.upgrade().map(Pipeline)
    }
}

#[derive(Debug)]
pub struct PipelineInner {
    app: RefCell<Option<GPSApp>>,
    pipeline: RefCell<Option<gst::Pipeline>>,
    current_state: Cell<PipelineState>,
}

impl Pipeline {
    pub fn new() -> anyhow::Result<Self> {
        let pipeline = Pipeline(Rc::new(PipelineInner {
            app: RefCell::new(None),
            pipeline: RefCell::new(None),
            current_state: Cell::new(PipelineState::Stopped),
        }));
        #[cfg(feature = "gtk4-plugin")]
        {
            gstgtk4::plugin_register_static().expect("Failed to register gstgtk4 plugin");
            ElementInfo::element_update_rank("gtk4paintablesink", gst::Rank::Primary);
        }

        Ok(pipeline)
    }

    pub fn set_app(&self, app: GPSAppWeak) {
        *self.app.borrow_mut() = Some(app.upgrade().unwrap());
    }

    pub fn create_pipeline(&self, description: &str) -> anyhow::Result<gst::Pipeline> {
        GPS_INFO!("Creating pipeline {}", description);

        // Create pipeline from the description
        let pipeline = gst::parse_launch(&description.to_string())?;
        let pipeline = pipeline.downcast::<gst::Pipeline>();
        /* start playing */
        if pipeline.is_err() {
            GPS_ERROR!("Can not create a proper pipeline from gstreamer parse_launch");
            return Err(anyhow::anyhow!(
                "Unable to create a pipeline from the given parse launch {"
            ));
        }
        Ok(pipeline.unwrap())
    }

    pub fn check_for_gtk4sink(&self, pipeline: &gst::Pipeline) {
        let bin = pipeline.clone().dynamic_cast::<gst::Bin>().unwrap();
        let gtksinks = ElementInfo::search_fo_element(&bin, "gtk4paintablesink");

        for (first_sink, gtksink) in gtksinks.into_iter().enumerate() {
            let paintable = gtksink.property::<gdk::Paintable>("paintable");
            self.app
                .borrow()
                .as_ref()
                .expect("App should be available")
                .set_app_preview(&paintable, first_sink);
        }
    }

    pub fn start_pipeline(
        &self,
        graphview: &GraphView,
        new_state: PipelineState,
    ) -> anyhow::Result<PipelineState> {
        if self.state() == PipelineState::Stopped || self.state() == PipelineState::Error {
            let pipeline = self
                .create_pipeline(&self.render_gst_launch(graphview))
                .map_err(|err| {
                    GPS_ERROR!("Unable to start a pipeline: {}", err);
                    err
                })?;

            let bus = pipeline.bus().expect("Pipeline had no bus");
            let pipeline_weak = self.downgrade();
            bus.add_watch_local(move |_bus, msg| {
                let pipeline = upgrade_weak!(pipeline_weak, glib::Continue(false));
                pipeline.on_pipeline_message(msg);
                glib::Continue(true)
            })?;
            pipeline.set_state(gst::State::Ready)?;
            self.check_for_gtk4sink(&pipeline);
            *self.pipeline.borrow_mut() = Some(pipeline);
        }

        self.set_state(new_state).map_err(|error| {
            GPS_ERROR!("Unable to change state {}", error);
            error
        })?;

        Ok(self.state())
    }

    pub fn set_state(&self, new_state: PipelineState) -> anyhow::Result<PipelineState> {
        if let Some(pipeline) = self.pipeline.borrow().to_owned() {
            match new_state {
                PipelineState::Playing => pipeline.set_state(gst::State::Playing)?,
                PipelineState::Paused => pipeline.set_state(gst::State::Paused)?,
                PipelineState::Stopped | PipelineState::Error => {
                    pipeline.set_state(gst::State::Null)?;
                    gst::StateChangeSuccess::Success
                }
            };
            self.current_state.set(new_state);
            self.app
                .borrow()
                .as_ref()
                .expect("App should be available")
                .set_app_state(Pipeline::state_to_app_state(new_state));
        }
        Ok(new_state)
    }

    pub fn state(&self) -> PipelineState {
        self.current_state.get()
    }

    pub fn set_position(&self, position: u64) -> anyhow::Result<()> {
        if let Some(pipeline) = self.pipeline.borrow().to_owned() {
            pipeline.seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                position * gst::ClockTime::SECOND,
            )?;
        }
        Ok(())
    }

    pub fn position(&self) -> u64 {
        let mut position = gst::ClockTime::NONE;
        if let Some(pipeline) = self.pipeline.borrow().to_owned() {
            position = pipeline.query_position::<gst::ClockTime>();
        }
        position.unwrap_or_default().mseconds()
    }

    pub fn duration(&self) -> u64 {
        let mut duration = gst::ClockTime::NONE;
        if let Some(pipeline) = self.pipeline.borrow().to_owned() {
            duration = pipeline.query_duration::<gst::ClockTime>();
        }
        duration.unwrap_or_default().mseconds()
    }

    pub fn position_description(&self) -> String {
        let mut position = gst::ClockTime::NONE;
        let mut duration = gst::ClockTime::NONE;
        if let Some(pipeline) = self.pipeline.borrow().to_owned() {
            position = pipeline.query_position::<gst::ClockTime>();
            duration = pipeline.query_duration::<gst::ClockTime>();
        }
        format!(
            "{:.0}/{:.0}",
            position.unwrap_or_default().display(),
            duration.unwrap_or_default().display(),
        )
    }

    fn state_to_app_state(state: PipelineState) -> AppState {
        match state {
            PipelineState::Playing => AppState::Playing,
            PipelineState::Paused => AppState::Paused,
            PipelineState::Stopped => AppState::Stopped,
            PipelineState::Error => AppState::Error,
        }
    }

    pub fn downgrade(&self) -> PipelineWeak {
        PipelineWeak(Rc::downgrade(&self.0))
    }

    fn on_pipeline_message(&self, msg: &gst::MessageRef) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Error(err) => {
                GPS_ERROR!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                self.set_state(PipelineState::Error)
                    .expect("Unable to set state to stopped");
            }
            MessageView::Application(msg) => match msg.structure() {
                // Here we can send ourselves messages from any thread and show them to the user in
                // the UI in case something goes wrong
                Some(s) if s.name() == "warning" => {
                    let text = s.get::<&str>("text").expect("Warning message without text");
                    GPS_WARN!("{}", text);
                }
                _ => (),
            },
            _ => (),
        };
    }

    // Render graph methods
    fn process_gst_node(
        &self,
        graphview: &GraphView,
        node: &Node,
        elements: &mut HashMap<String, String>,
        mut description: String,
    ) -> String {
        let unique_name = node.unique_name();
        description.push_str(&format!("{} name={} ", node.name(), unique_name));
        elements.insert(unique_name.clone(), unique_name.clone());
        for (name, value) in node.properties().iter() {
            //This allow to have an index in front of a property such as an enum.
            let value = value.split_once(':').unwrap_or((value, value));
            if !node.hidden_property(name) {
                description.push_str(&format!("{}={} ", name, value.1));
            }
        }

        let ports = node.all_ports(PortDirection::Output);
        let n_ports = ports.len();
        for port in ports {
            if let Some((_port_to, node_to)) = graphview.port_connected_to(port.id()) {
                if n_ports > 1 {
                    description.push_str(&format!("{}. ! ", unique_name));
                } else {
                    description.push_str("! ");
                }
                if let Some(node) = graphview.node(node_to) {
                    if elements.contains_key(&node.unique_name()) {
                        description.push_str(&format!("{}. ", node.unique_name()));
                    } else {
                        description =
                            self.process_gst_node(graphview, &node, elements, description.clone());
                    }
                }
            }
        }
        description
    }

    pub fn render_gst_launch(&self, graphview: &GraphView) -> String {
        let source_nodes = graphview.all_nodes(NodeType::Source);
        let mut elements: HashMap<String, String> = HashMap::new();
        let mut description = String::from("");
        for source_node in source_nodes {
            description =
                self.process_gst_node(graphview, &source_node, &mut elements, description.clone());
        }
        description
    }
}

impl Drop for PipelineInner {
    fn drop(&mut self) {
        // TODO: If a recording is currently running we would like to finish that first
        // before quitting the pipeline and shutting down the pipeline.
        if let Some(pipeline) = self.pipeline.borrow().to_owned() {
            // We ignore any errors here
            let _ = pipeline.set_state(gst::State::Null);

            // Remove the message watch from the bus
            let bus = pipeline.bus().expect("Pipeline had no bus");
            let _ = bus.remove_watch();
        }
    }
}
