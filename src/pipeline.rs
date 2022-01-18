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
use crate::graphmanager::{GraphView, Node, NodeType, PortDirection};
use crate::logger;
use crate::GPS_INFO;

use gst::glib;
use gst::prelude::*;
use gstreamer as gst;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
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
    pipeline: RefCell<Option<gst::Pipeline>>,
    current_state: Cell<PipelineState>,
}

impl Pipeline {
    pub fn new() -> anyhow::Result<Self> {
        let pipeline = Pipeline(Rc::new(PipelineInner {
            pipeline: RefCell::new(None),
            current_state: Cell::new(PipelineState::Stopped),
        }));

        Ok(pipeline)
    }

    pub fn create_pipeline(&self, description: &str) -> anyhow::Result<()> {
        GPS_INFO!("Creating pipeline {}", description);

        // Create pipeline from the description
        let pipeline = gst::parse_launch(&description.to_string())?;
        if let Ok(pipeline) = pipeline.downcast::<gst::Pipeline>() {
            let bus = pipeline.bus().expect("Pipeline had no bus");
            let pipeline_weak = self.downgrade();
            bus.add_watch_local(move |_bus, msg| {
                let pipeline = upgrade_weak!(pipeline_weak, glib::Continue(false));
                pipeline.on_pipeline_message(msg);
                glib::Continue(true)
            })?;

            *self.pipeline.borrow_mut() = Some(pipeline);
            /* start playing */
        } else {
            GPS_ERROR!("Can not create a proper pipeline from gstreamer parse_launch");
        }
        Ok(())
    }

    pub fn start_pipeline(
        &self,
        graphview: &GraphView,
        new_state: PipelineState,
    ) -> anyhow::Result<PipelineState> {
        if self.state() == PipelineState::Stopped {
            self.create_pipeline(&self.render_gst_launch(graphview))
                .map_err(|err| {
                    GPS_ERROR!("Unable to start a pipeline: {}", err);
                    err
                })?;
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
                PipelineState::Stopped => {
                    pipeline.set_state(gst::State::Null)?;
                    gst::StateChangeSuccess::Success
                }
            };
            self.current_state.set(new_state);
        }
        Ok(new_state)
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
                GPS_ERROR!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
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
                    GPS_WARN!("{}", text);
                }
                _ => (),
            },
            _ => (),
        };
    }

    pub fn elements_list() -> anyhow::Result<Vec<ElementInfo>> {
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

    fn element_feature(element_name: &str) -> Option<gst::PluginFeature> {
        let registry = gst::Registry::get();
        gst::Registry::find_feature(&registry, element_name, gst::ElementFactory::static_type())
    }

    pub fn element_description(element_name: &str) -> anyhow::Result<String> {
        let mut desc = String::from("");
        let feature = Pipeline::element_feature(element_name)
            .ok_or_else(|| glib::bool_error!("Failed get element feature"))?;

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

    fn value_as_str(v: &glib::Value) -> Option<String> {
        match v.type_() {
            glib::Type::I8 => Some(str_some_value!(v, i8).to_string()),
            glib::Type::U8 => Some(str_some_value!(v, u8).to_string()),
            glib::Type::BOOL => Some(str_some_value!(v, bool).to_string()),
            glib::Type::I32 => Some(str_some_value!(v, i32).to_string()),
            glib::Type::U32 => Some(str_some_value!(v, u32).to_string()),
            glib::Type::I64 => Some(str_some_value!(v, i64).to_string()),
            glib::Type::U64 => Some(str_some_value!(v, u64).to_string()),
            glib::Type::F32 => Some(str_some_value!(v, f32).to_string()),
            glib::Type::F64 => Some(str_some_value!(v, f64).to_string()),
            glib::Type::STRING => str_opt_value!(v, String),
            _ => None,
        }
    }

    pub fn element_properties(element_name: &str) -> anyhow::Result<HashMap<String, String>> {
        let mut properties_list = HashMap::new();
        let feature = Pipeline::element_feature(element_name).expect("Unable to get feature");

        let factory = feature
            .downcast::<gst::ElementFactory>()
            .expect("Unable to get the factory from the feature");
        let element = factory.create(None)?;
        let params = element.class().list_properties();

        for param in params {
            GPS_INFO!("Property_name {}", param.name());
            if (param.flags() & glib::ParamFlags::READABLE) == glib::ParamFlags::READABLE
                || (param.flags() & glib::ParamFlags::READWRITE) == glib::ParamFlags::READWRITE
            {
                let value = Pipeline::value_as_str(&element.property(param.name()).unwrap())
                    .unwrap_or_else(|| String::from(""));
                properties_list.insert(String::from(param.name()), value);
            } else if let Some(value) = Pipeline::value_as_str(param.default_value()) {
                properties_list.insert(String::from(param.name()), value);
            } else {
                GPS_INFO!("Unable to add property_name {}", param.name());
            }
        }
        Ok(properties_list)
    }

    pub fn element_is_uri_src_handler(element_name: &str) -> bool {
        let feature = Pipeline::element_feature(element_name).expect("Unable to get feature");

        let factory = feature
            .downcast::<gst::ElementFactory>()
            .expect("Unable to get the factory from the feature");
        let element = factory
            .create(None)
            .expect("Unable to create an element from the feature");
        match element.dynamic_cast::<gst::URIHandler>() {
            Ok(uri_handler) => uri_handler.uri_type() == gst::URIType::Src,
            Err(_e) => false,
        }
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
            if !node.hidden_property(name) {
                description.push_str(&format!("{}={} ", name, value));
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
