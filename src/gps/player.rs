// player.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::{AppState, GPSApp, GPSAppWeak};
use crate::graphmanager as GM;
use crate::graphmanager::PropertyExt;

use crate::common;
use crate::gps::ElementInfo;
use crate::logger;
use crate::settings;
use crate::GPS_INFO;

use gst::glib;
use gst::prelude::*;
use gtk::gdk;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write as _;
use std::ops;
use std::rc::{Rc, Weak};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    Playing,
    Paused,
    Stopped,
    Error,
}

impl fmt::Display for PipelineState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone)]
pub struct Player(Rc<PlayerInner>);

// Deref into the contained struct to make usage a bit more ergonomic
impl ops::Deref for Player {
    type Target = PlayerInner;

    fn deref(&self) -> &PlayerInner {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct PlayerWeak(Weak<PlayerInner>);

impl PlayerWeak {
    pub fn upgrade(&self) -> Option<Player> {
        self.0.upgrade().map(Player)
    }
}

#[derive(Debug)]
pub struct PlayerInner {
    app: RefCell<Option<GPSApp>>,
    pipeline: RefCell<Option<gst::Pipeline>>,
    current_state: Cell<PipelineState>,
    n_video_sink: Cell<usize>,
    bus_watch_guard: RefCell<Option<gst::bus::BusWatchGuard>>,
}

impl Player {
    pub fn new() -> anyhow::Result<Self> {
        let pipeline = Player(Rc::new(PlayerInner {
            app: RefCell::new(None),
            pipeline: RefCell::new(None),
            current_state: Cell::new(PipelineState::Stopped),
            n_video_sink: Cell::new(0),
            bus_watch_guard: RefCell::new(None),
        }));

        Ok(pipeline)
    }

    pub fn get_version() -> String {
        gst::version_string().to_string()
    }

    pub fn set_app(&self, app: GPSAppWeak) {
        *self.app.borrow_mut() = Some(app.upgrade().unwrap());
    }

    pub fn create_pipeline(&self, description: &str) -> anyhow::Result<gst::Pipeline> {
        GPS_INFO!("Creating pipeline {}", description);
        self.n_video_sink.set(0);
        if settings::Settings::load_settings()
            .preferences
            .get("use_gtk4_sink")
            .unwrap_or(&"true".to_string())
            .parse::<bool>()
            .expect("Should a boolean value")
        {
            ElementInfo::element_update_rank("gtk4paintablesink", gst::Rank::Primary);
        } else {
            ElementInfo::element_update_rank("gtk4paintablesink", gst::Rank::Marginal);
        }

        // Create pipeline from the description
        let pipeline = gst::parse_launch(description)?;
        let pipeline = pipeline.downcast::<gst::Pipeline>();
        /* start playing */
        if pipeline.is_err() {
            GPS_ERROR!("Can not create a proper pipeline from gstreamer parse_launch");
            return Err(anyhow::anyhow!(
                "Unable to create a pipeline from the given parse launch"
            ));
        }
        self.check_for_gtk4sink(pipeline.as_ref().unwrap());
        // GPSApp is not Send(trait) ready , so we use a channel to exchange the given data with the main thread and use
        // GPSApp.
        let (ready_tx, ready_rx) = glib::MainContext::channel(glib::Priority::DEFAULT);
        let player_weak = self.downgrade();
        let _ = ready_rx.attach(None, move |element: gst::Element| {
            let player = upgrade_weak!(player_weak, glib::ControlFlow::Break);
            let paintable = element.property::<gdk::Paintable>("paintable");
            let n_sink = player.n_video_sink.get();
            player
                .app
                .borrow()
                .as_ref()
                .expect("App should be available")
                .set_app_preview(&paintable, n_sink);
            player.n_video_sink.set(n_sink + 1);
            glib::ControlFlow::Continue
        });
        let bin = pipeline.unwrap().dynamic_cast::<gst::Bin>();
        if let Ok(bin) = bin.as_ref() {
            bin.connect_deep_element_added(move |_, _, element| {
                if let Some(factory) = element.factory() {
                    GPS_INFO!("Received the signal deep element added {}", factory.name());
                    if factory.name() == "gtk4paintablesink" {
                        let _ = ready_tx.send(element.clone());
                    }
                }
            });
        }
        let pipeline = bin.unwrap().dynamic_cast::<gst::Pipeline>();
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
        graphview: &GM::GraphView,
        new_state: PipelineState,
    ) -> anyhow::Result<PipelineState> {
        if self.state() == PipelineState::Stopped || self.state() == PipelineState::Error {
            let pipeline = self
                .create_pipeline(&self.pipeline_description_from_graphview(graphview))
                .map_err(|err| {
                    GPS_ERROR!("Unable to create a pipeline: {}", err);
                    err
                })?;

            let bus = pipeline.bus().expect("Pipeline had no bus");
            let pipeline_weak = self.downgrade();
            let bus_watch_guard = bus.add_watch_local(move |_bus, msg| {
                let pipeline = upgrade_weak!(pipeline_weak, glib::ControlFlow::Break);
                pipeline.on_pipeline_message(msg);
                glib::ControlFlow::Continue
            })?;
            *self.pipeline.borrow_mut() = Some(pipeline);
            *self.bus_watch_guard.borrow_mut() = Some(bus_watch_guard);
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
                    self.n_video_sink.set(0);
                    gst::StateChangeSuccess::Success
                }
            };
            self.current_state.set(new_state);
            self.app
                .borrow()
                .as_ref()
                .expect("App should be available")
                .set_app_state(Player::state_to_app_state(new_state));
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

    pub fn playing(&self) -> bool {
        self.state() == PipelineState::Playing || self.state() == PipelineState::Paused
    }
    pub fn n_video_sink(&self) -> usize {
        self.n_video_sink.get()
    }

    pub fn downgrade(&self) -> PlayerWeak {
        PlayerWeak(Rc::downgrade(&self.0))
    }

    fn on_pipeline_message(&self, msg: &gst::MessageRef) {
        use gst::MessageView;
        GPS_MSG!("{:?}", msg.structure());
        match msg.view() {
            MessageView::Eos(_) => {
                GPS_INFO!("EOS received");
                self.set_state(PipelineState::Stopped)
                    .expect("Unable to set state to stopped");
            }
            MessageView::Error(err) => {
                GPS_ERROR!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                self.set_state(PipelineState::Error)
                    .expect("Unable to set state to Error");
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

    pub fn pipeline_elements(&self) -> Option<Vec<String>> {
        if self.playing() {
            let bin = self
                .pipeline
                .borrow()
                .clone()
                .unwrap()
                .dynamic_cast::<gst::Bin>()
                .unwrap();
            let elements_name: Vec<String> = ElementInfo::search_fo_element(&bin, "")
                .iter()
                .map(|e| e.factory().unwrap().name().to_string())
                .collect();
            return Some(elements_name);
        }
        None
    }

    // Render graph methods
    #[allow(clippy::only_used_in_recursion)]
    fn process_gst_node(
        &self,
        graphview: &GM::GraphView,
        node: &GM::Node,
        elements: &mut HashMap<String, String>,
        mut description: String,
    ) -> String {
        let unique_name = node.unique_name();
        let _ = write!(description, "{} name={} ", node.name(), unique_name);
        elements.insert(unique_name.clone(), unique_name.clone());
        // Node properties
        for (name, value) in node.properties().iter() {
            //This allow to have an index in front of a property such as an enum.
            if !node.hidden_property(name) {
                let _ = write!(description, "{name}={value} ");
            }
        }
        //Port properties
        let ports = node.all_ports(GM::PortDirection::All);
        for port in ports {
            for (name, value) in port.properties().iter() {
                if !port.hidden_property(name) {
                    let _ = write!(description, "{}::{}={} ", port.name(), name, value);
                }
            }
        }

        let ports = node.all_ports(GM::PortDirection::Output);
        let n_ports = ports.len();
        for port in ports {
            if let Some((_port_to, node_to)) = graphview.port_connected_to(port.id()) {
                if n_ports > 1 {
                    let _ = write!(description, "{unique_name}. ! ");
                } else {
                    if let Some(link) = graphview.port_link(port.id()) {
                        if !link.name().is_empty() {
                            let _ = write!(description, "! {} ", link.name());
                        }
                    }
                    description.push_str("! ");
                }
                if let Some(node) = graphview.node(node_to) {
                    if elements.contains_key(&node.unique_name()) {
                        let _ = write!(description, "{}. ", node.unique_name());
                    } else {
                        description =
                            self.process_gst_node(graphview, &node, elements, description.clone());
                    }
                }
            }
        }
        description
    }

    pub fn pipeline_description_from_graphview(&self, graphview: &GM::GraphView) -> String {
        let source_nodes = graphview.all_nodes(GM::NodeType::Source);
        let mut elements: HashMap<String, String> = HashMap::new();
        let mut description = String::from("");
        for source_node in source_nodes {
            description =
                self.process_gst_node(graphview, &source_node, &mut elements, description.clone());
        }
        description
    }

    pub fn create_links_for_element(&self, element: &gst::Element, graphview: &GM::GraphView) {
        let mut iter = element.iterate_pads();
        let node = graphview
            .node_by_unique_name(&element.name())
            .expect("node should exists");

        loop {
            match iter.next() {
                Ok(Some(pad)) => {
                    GPS_INFO!("Found pad: {}", pad.name());

                    if pad.direction() == gst::PadDirection::Src {
                        let port = node
                            .port_by_name(&pad.name())
                            .expect("The port should exist here");
                        if let Some(peer_pad) = pad.peer() {
                            if let Some(peer_element) = peer_pad.parent_element() {
                                let peer_node = graphview
                                    .node_by_unique_name(&peer_element.name())
                                    .expect("The node should exists here");
                                let peer_port = peer_node
                                    .port_by_name(&peer_pad.name())
                                    .expect("The port should exists here");
                                self.app.borrow().as_ref().unwrap().create_link(
                                    node.id(),
                                    peer_node.id(),
                                    port.id(),
                                    peer_port.id(),
                                );
                            }
                        }
                    }
                }
                Err(gst::IteratorError::Resync) => iter.resync(),
                _ => break,
            }
        }
    }

    pub fn create_pads_for_element(&self, element: &gst::Element, node: &GM::Node) {
        let mut iter = element.iterate_pads();
        loop {
            match iter.next() {
                Ok(Some(pad)) => {
                    let pad_name = pad.name().to_string();
                    GPS_INFO!("Found pad: {}", pad_name);
                    let mut port_direction = GM::PortDirection::Input;
                    if pad.direction() == gst::PadDirection::Src {
                        port_direction = GM::PortDirection::Output;
                    }
                    let port_id = self.app.borrow().as_ref().unwrap().create_port_with_caps(
                        node.id(),
                        port_direction,
                        GM::PortPresence::Always,
                        pad.current_caps()
                            .unwrap_or_else(|| pad.query_caps(None))
                            .to_string(),
                    );
                    if let Some(port) = node.port(port_id) {
                        port.set_name(&pad_name);
                    }
                }
                Err(gst::IteratorError::Resync) => iter.resync(),
                _ => break,
            }
        }
    }

    pub fn create_properties_for_element(&self, element: &gst::Element, node: &GM::Node) {
        let properties = ElementInfo::element_properties(element)
            .unwrap_or_else(|_| panic!("Couldn't get properties for {}", node.name()));
        for (property_name, property_value) in properties {
            if property_name == "name"
                || property_name == "parent"
                || (property_value.flags() & glib::ParamFlags::READABLE)
                    != glib::ParamFlags::READABLE
            {
                continue;
            }

            if let Ok(value_str) = ElementInfo::element_property(element, &property_name) {
                let default_value_str =
                    common::value_as_str(property_value.default_value()).unwrap_or_default();
                GPS_DEBUG!(
                    "property name {} value_str '{}' default '{}'",
                    property_name,
                    value_str,
                    default_value_str
                );
                if !value_str.is_empty() && value_str != default_value_str {
                    node.add_property(&property_name, &value_str);
                }
            }
        }
    }

    pub fn graphview_from_pipeline_description(
        &self,
        graphview: &GM::GraphView,
        pipeline_desc: &str,
    ) {
        graphview.clear();

        if let Ok(pipeline) = self.create_pipeline(pipeline_desc) {
            let mut iter = pipeline.iterate_elements();
            let mut elements: Vec<gst::Element> = Vec::new();
            let elements = loop {
                match iter.next() {
                    Ok(Some(element)) => {
                        GPS_INFO!("Found element: {}", element.name());
                        let element_factory_name = element.factory().unwrap().name().to_string();
                        let node = graphview.create_node(
                            &element_factory_name,
                            ElementInfo::element_type(&element_factory_name),
                        );
                        node.set_unique_name(&element.name());
                        graphview.add_node(node.clone());
                        self.create_pads_for_element(&element, &node);
                        self.create_properties_for_element(&element, &node);
                        elements.push(element);
                    }
                    Err(gst::IteratorError::Resync) => iter.resync(),
                    _ => break elements,
                }
            };
            for element in elements {
                self.create_links_for_element(&element, graphview);
            }
        } else {
            GPS_ERROR!("Unable to create a pipeline: {}", pipeline_desc);
        }
    }
}

impl Drop for PlayerInner {
    fn drop(&mut self) {
        if let Some(pipeline) = self.pipeline.borrow().to_owned() {
            // We ignore any errors here
            let _ = pipeline.set_state(gst::State::Null);
        }
    }
}
