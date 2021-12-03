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
use crate::app::GPSApp;
use crate::graphmanager::NodeType;
use gst::prelude::*;
use gstreamer as gst;
use std::cell::{Cell, RefCell};
use std::error;
use std::fmt;
use std::ops;
use std::rc::{Rc, Weak};

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ElementInfo {
    pub name: Option<String>,
    plugin_name: Option<String>,
    rank: i32,
}

impl Default for ElementInfo {
    fn default() -> ElementInfo {
        ElementInfo {
            name: None,
            plugin_name: None,
            rank: -1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PipelineState {
    Playing,
    Paused,
    Stopped,
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
    initialized: bool,
    pipeline: RefCell<Option<gst::Pipeline>>,
    current_state: Cell<PipelineState>,
}

impl Pipeline {
    pub fn new() -> Result<Self, Box<dyn error::Error>> {
        gst::init()?;
        let pipeline = Pipeline(Rc::new(PipelineInner {
            initialized: true,
            pipeline: RefCell::new(None),
            current_state: Cell::new(PipelineState::Stopped),
        }));

        Ok(pipeline)
    }

    pub fn create_pipeline(&self, description: &str) -> Result<(), Box<dyn error::Error>> {
        println!("Creating pipeline {}", description);

        /* create playbin */

        let pipeline = gst::parse_launch(&description.to_string())?;
        let pipeline = pipeline
            .downcast::<gst::Pipeline>()
            .expect("Couldn't downcast pipeline");

        //pipeline.set_property_message_forward(true);

        let bus = pipeline.bus().expect("Pipeline had no bus");
        let pipeline_weak = self.downgrade();
        bus.add_watch_local(move |_bus, msg| {
            let pipeline = upgrade_weak!(pipeline_weak, glib::Continue(false));

            pipeline.on_pipeline_message(msg);

            glib::Continue(true)
        })?;

        *self.pipeline.borrow_mut() = Some(pipeline);
        /* start playing */

        Ok(())
    }

    pub fn set_state(&self, state: PipelineState) -> Result<(), Box<dyn error::Error>> {
        if let Some(pipeline) = self.pipeline.borrow().to_owned() {
            match state {
                PipelineState::Playing => pipeline.set_state(gst::State::Playing)?,
                PipelineState::Paused => pipeline.set_state(gst::State::Paused)?,
                PipelineState::Stopped => {
                    pipeline.set_state(gst::State::Null)?;
                    gst::StateChangeSuccess::Success
                }
            };
            self.current_state.set(state);
        }
        Ok(())
    }

    pub fn state(&self) -> PipelineState {
        self.current_state.get()
    }

    pub fn downgrade(&self) -> PipelineWeak {
        PipelineWeak(Rc::downgrade(&self.0))
    }

    fn on_pipeline_message(&self, msg: &gst::MessageRef) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Error(err) => {
                GPSApp::show_error_dialog(
                    false,
                    format!(
                        "Error from {:?}: {} ({:?})",
                        err.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    )
                    .as_str(),
                );
            }
            MessageView::Application(msg) => match msg.structure() {
                // Here we can send ourselves messages from any thread and show them to the user in
                // the UI in case something goes wrong
                Some(s) if s.name() == "warning" => {
                    let text = s.get::<&str>("text").expect("Warning message without text");
                    GPSApp::show_error_dialog(false, text);
                }
                _ => (),
            },
            _ => (),
        };
    }

    pub fn elements_list() -> Result<Vec<ElementInfo>, Box<dyn error::Error>> {
        let registry = gst::Registry::get();
        let mut elements: Vec<ElementInfo> = Vec::new();
        let plugins = gst::Registry::plugin_list(&registry);
        for plugin in plugins {
            let plugin_name = gst::Plugin::plugin_name(&plugin);
            let features = gst::Registry::feature_list_by_plugin(&registry, &plugin_name);
            for feature in features {
                let mut element = ElementInfo::default();
                if let Ok(factory) = feature.downcast::<gst::ElementFactory>() {
                    let feature = factory.upcast::<gst::PluginFeature>();

                    element.name = Some(gst::PluginFeature::name(&feature).as_str().to_owned());
                    element.plugin_name =
                        Some(gst::Plugin::plugin_name(&plugin).as_str().to_owned());
                    elements.push(element);
                }
            }
        }
        elements.sort();
        Ok(elements)
    }

    pub fn element_feature(
        element_name: &str,
    ) -> anyhow::Result<gst::PluginFeature, Box<dyn error::Error>> {
        let registry = gst::Registry::get();
        let feature = gst::Registry::find_feature(
            &registry,
            element_name,
            gst::ElementFactory::static_type(),
        )
        .expect("Unable to find the element name");
        Ok(feature)
    }

    pub fn element_description(
        element_name: &str,
    ) -> anyhow::Result<String, Box<dyn error::Error>> {
        let mut desc = String::from("");
        let feature = Pipeline::element_feature(element_name)?;

        if let Ok(factory) = feature.downcast::<gst::ElementFactory>() {
            desc.push_str("<b>Factory details:</b>\n");
            desc.push_str("<b>Name:</b>");
            desc.push_str(&factory.name());
            desc.push('\n');

            let element_keys = factory.metadata_keys();
            for key in element_keys {
                let val = factory.metadata(&key);
                if let Some(val) = val {
                    desc.push_str("<b>");
                    desc.push_str(&key);
                    desc.push_str("</b>:");
                    desc.push_str(&gtk::glib::markup_escape_text(&val).to_string());
                    desc.push('\n');
                }
            }
            let feature = factory.upcast::<gst::PluginFeature>();
            let plugin = gst::PluginFeature::plugin(&feature);
            if let Some(plugin) = plugin {
                desc.push('\n');
                desc.push_str("<b>Plugin details:</b>");
                desc.push('\n');
                desc.push_str("<b>Name:");
                desc.push_str("</b>");
                desc.push_str(gst::Plugin::plugin_name(&plugin).as_str());
                desc.push('\n');
                desc.push_str("<b>Description:");
                desc.push_str("</b>");
                desc.push_str(&gtk::glib::markup_escape_text(&plugin.description()).to_string());
                desc.push('\n');
                desc.push_str("<b>Filename:");
                desc.push_str("</b>");
                desc.push_str(
                    &gtk::glib::markup_escape_text(
                        &plugin.filename().unwrap().as_path().display().to_string(),
                    )
                    .to_string(),
                );
                desc.push('\n');
                desc.push_str("<b>Version:");
                desc.push_str("</b>");
                desc.push_str(&gtk::glib::markup_escape_text(&plugin.version()).to_string());
                desc.push('\n');
            }
        }
        Ok(desc)
    }

    pub fn pads(element_name: &str, include_on_request: bool) -> (u32, u32) {
        let feature = Pipeline::element_feature(element_name).expect("Unable to get feature");
        let mut input = 0;
        let mut output = 0;

        if let Ok(factory) = feature.downcast::<gst::ElementFactory>() {
            if factory.num_pad_templates() > 0 {
                let pads = factory.static_pad_templates();
                for pad in pads {
                    if pad.presence() == gst::PadPresence::Always
                        || (include_on_request
                            && (pad.presence() == gst::PadPresence::Request
                                || pad.presence() == gst::PadPresence::Sometimes))
                    {
                        if pad.direction() == gst::PadDirection::Src {
                            output += 1;
                        } else if pad.direction() == gst::PadDirection::Sink {
                            input += 1;
                        }
                    }
                }
            }
        }
        (input, output)
    }

    pub fn element_type(element_name: &str) -> NodeType {
        let pads = Pipeline::pads(element_name, true);
        let mut element_type = NodeType::Source;
        if pads.0 > 0 {
            if pads.1 > 0 {
                element_type = NodeType::Transform;
            } else {
                element_type = NodeType::Sink;
            }
        } else if pads.1 > 0 {
            element_type = NodeType::Source;
        }

        element_type
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
