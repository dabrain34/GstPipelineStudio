// player.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::settings;
use crate::app::{AppState, GPSApp, GPSAppWeak};
use crate::common;
use crate::gps::ElementInfo;
use crate::graphmanager as GM;
use crate::graphmanager::PropertyExt;
use crate::logger;

use gst::glib;
use gst::prelude::*;
use gtk::gdk;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fmt;
use std::fmt::Write as _;
use std::ops;
use std::rc::{Rc, Weak};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    Playing,
    Paused,
    #[default]
    Stopped,
    Error,
}

impl fmt::Display for PipelineState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone, Default)]
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

fn gst_log_handler(
    category: gst::DebugCategory,
    level: gst::DebugLevel,
    file: &glib::GStr,
    function: &glib::GStr,
    line: u32,
    _obj: Option<&gst::LoggedObject>,
    message: &gst::DebugMessage,
) {
    if let Some(msg) = message.get() {
        let log_message = format!(
            "{}\t{}\t{}:{}:{}\t{}",
            level,
            category.name(),
            line,
            file.as_str(),
            function.as_str(),
            msg.as_str()
        );
        GPS_GST_LOG!("{}", log_message);
    }
}

#[derive(Debug, Default)]
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
        gst::log::add_log_function(gst_log_handler);
        Ok(pipeline)
    }

    pub fn version() -> String {
        let version_string = gst::version_string().to_string();
        // Extract just the version part after "GStreamer Library "
        version_string
            .trim_start_matches("GStreamer Library ")
            .trim_start_matches("GStreamer ")
            .to_string()
    }

    pub fn set_app(&self, app: GPSAppWeak) -> anyhow::Result<()> {
        let upgraded_app = app
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Failed to upgrade app weak reference"))?;
        *self.app.borrow_mut() = Some(upgraded_app);
        Ok(())
    }

    /// Helper method to access the app with proper error handling
    fn with_app<F, R>(&self, f: F) -> anyhow::Result<R>
    where
        F: FnOnce(&GPSApp) -> R,
    {
        let app = self.app.borrow();
        let app = app
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("App not initialized"))?;
        Ok(f(app))
    }

    pub fn create_pipeline(&self, description: &str) -> anyhow::Result<gst::Pipeline> {
        GPS_INFO!("Creating pipeline {}", description);
        self.n_video_sink.set(0);
        let use_gtk4_sink = settings::Settings::load_settings()
            .preferences
            .get("use_gtk4_sink")
            .unwrap_or(&"true".to_string())
            .parse::<bool>()
            .unwrap_or(true); // Default to true if invalid value

        if use_gtk4_sink {
            ElementInfo::element_update_rank("gtk4paintablesink", gst::Rank::PRIMARY);
        } else {
            ElementInfo::element_update_rank("gtk4paintablesink", gst::Rank::MARGINAL);
        }
        gst::log::set_threshold_from_string(settings::Settings::gst_log_level().as_str(), true);
        // Create pipeline from the description
        let pipeline = gst::parse::launch(description)?;
        let pipeline = pipeline.downcast::<gst::Pipeline>().map_err(|_| {
            GPS_ERROR!("Cannot create a proper pipeline from gstreamer parse_launch");
            anyhow::anyhow!("Unable to create a pipeline from the given parse launch")
        })?;

        self.check_for_gtk4sink(&pipeline)?;
        // GPSApp is not Send(trait) ready , so we use a channel to exchange the given data with the main thread and use
        // GPSApp.
        let (ready_tx, ready_rx) = async_channel::unbounded::<gst::Element>();
        let player_weak = self.downgrade();
        glib::spawn_future_local(async move {
            while let Ok(element) = ready_rx.recv().await {
                let player = upgrade_weak!(player_weak, glib::ControlFlow::Break);
                let paintable = element.property::<gdk::Paintable>("paintable");
                let n_sink = player.n_video_sink.get();
                if let Err(e) = player.with_app(|app| app.set_app_preview(&paintable, n_sink)) {
                    GPS_ERROR!("Failed to set app preview: {}", e);
                    return glib::ControlFlow::Break;
                }
                player.n_video_sink.set(n_sink + 1);
            }
            glib::ControlFlow::Continue
        });
        let bin = pipeline
            .dynamic_cast::<gst::Bin>()
            .map_err(|_| anyhow::anyhow!("Pipeline cannot be cast to Bin"))?;

        bin.connect_deep_element_added(move |_, _, element| {
            if let Some(factory) = element.factory() {
                GPS_INFO!("Received the signal deep element added {}", factory.name());
                if factory.name() == "gtk4paintablesink" {
                    let _ = ready_tx.try_send(element.clone());
                }
            }
        });

        let pipeline = bin
            .dynamic_cast::<gst::Pipeline>()
            .map_err(|_| anyhow::anyhow!("Bin cannot be cast to Pipeline"))?;
        Ok(pipeline)
    }

    pub fn check_for_gtk4sink(&self, pipeline: &gst::Pipeline) -> anyhow::Result<()> {
        let bin = pipeline
            .clone()
            .dynamic_cast::<gst::Bin>()
            .map_err(|_| anyhow::anyhow!("Pipeline cannot be cast to Bin"))?;
        let gtksinks = ElementInfo::search_for_element(&bin, "gtk4paintablesink");

        for (first_sink, gtksink) in gtksinks.into_iter().enumerate() {
            let paintable = gtksink.property::<gdk::Paintable>("paintable");
            self.with_app(|app| app.set_app_preview(&paintable, first_sink))?;
        }
        Ok(())
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

            let bus = pipeline
                .bus()
                .ok_or_else(|| anyhow::anyhow!("Pipeline has no bus"))?;
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
        self.set_state_with_message(new_state, None)
    }

    pub fn set_state_with_message(
        &self,
        new_state: PipelineState,
        error_message: Option<String>,
    ) -> anyhow::Result<PipelineState> {
        if let Some(pipeline) = self.pipeline.borrow().to_owned() {
            match new_state {
                PipelineState::Playing => {
                    pipeline.set_state(gst::State::Playing)?;
                }
                PipelineState::Paused => {
                    pipeline.set_state(gst::State::Paused)?;
                }
                PipelineState::Stopped | PipelineState::Error => {
                    pipeline.set_state(gst::State::Null)?;
                    self.n_video_sink.set(0);
                }
            }
            self.current_state.set(new_state);
            let app_state = match new_state {
                PipelineState::Playing => AppState::Playing,
                PipelineState::Paused => AppState::Paused,
                PipelineState::Stopped => AppState::Stopped,
                PipelineState::Error => AppState::Error(error_message),
            };
            self.with_app(|app| app.set_app_state(app_state))?;
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
        self.pipeline
            .borrow()
            .as_ref()
            .and_then(|p| p.query_position::<gst::ClockTime>())
            .unwrap_or_default()
            .mseconds()
    }

    pub fn duration(&self) -> u64 {
        self.pipeline
            .borrow()
            .as_ref()
            .and_then(|p| p.query_duration::<gst::ClockTime>())
            .unwrap_or_default()
            .mseconds()
    }

    pub fn position_description(&self) -> String {
        let (position, duration) = if let Some(pipeline) = self.pipeline.borrow().as_ref() {
            (
                pipeline.query_position::<gst::ClockTime>(),
                pipeline.query_duration::<gst::ClockTime>(),
            )
        } else {
            (None, None)
        };

        format!(
            "{:.0}/{:.0}",
            position.unwrap_or_default().display(),
            duration.unwrap_or_default().display(),
        )
    }

    pub fn is_playing(&self) -> bool {
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

        // Get source element name - use borrowed str to avoid allocations
        let src_name = msg
            .src()
            .map(|s| s.name())
            .unwrap_or_else(|| glib::GString::from("unknown"));

        // Format: src\tmessage_type\tdetails
        match msg.view() {
            MessageView::Eos(_) => {
                GPS_MSG_LOG!("{}\tEos\tEnd of stream", src_name);
                GPS_INFO!("EOS received");
                if let Err(e) = self.set_state(PipelineState::Stopped) {
                    GPS_ERROR!("Failed to set stopped state: {}", e);
                }
            }
            MessageView::Error(err) => {
                GPS_MSG_LOG!("{}\tError\t{}", src_name, err.error());
                GPS_ERROR!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(gst::Object::path_string),
                    err.error(),
                    err.debug()
                );
                let error_msg = format!("{}: {}", src_name, err.error());
                if let Err(e) = self.set_state_with_message(PipelineState::Error, Some(error_msg)) {
                    GPS_ERROR!("Failed to set error state: {}", e);
                }
            }
            MessageView::Warning(warn) => {
                GPS_MSG_LOG!("{}\tWarning\t{}", src_name, warn.error());
            }
            MessageView::Info(info) => {
                GPS_MSG_LOG!("{}\tInfo\t{}", src_name, info.error());
            }
            MessageView::StateChanged(sc) => {
                GPS_MSG_LOG!(
                    "{}\tStateChanged\t{:?} → {:?}",
                    src_name,
                    sc.old(),
                    sc.current()
                );
            }
            MessageView::StreamStatus(ss) => {
                let (status_type, owner) = ss.get();
                let owner_name = owner.name();
                GPS_MSG_LOG!(
                    "{}\tStreamStatus\t{:?} (owner: {})",
                    src_name,
                    status_type,
                    owner_name
                );
            }
            MessageView::Tag(tag) => {
                let tags = tag.tags();
                GPS_MSG_LOG!("{}\tTag\t{} tag(s)", src_name, tags.n_tags());
            }
            MessageView::Buffering(buf) => {
                GPS_MSG_LOG!("{}\tBuffering\t{}%", src_name, buf.percent());
            }
            MessageView::DurationChanged(_) => {
                GPS_MSG_LOG!("{}\tDurationChanged\tDuration changed", src_name);
            }
            MessageView::AsyncDone(_) => {
                GPS_MSG_LOG!("{}\tAsyncDone\tAsync operation completed", src_name);
            }
            MessageView::Latency(_) => {
                GPS_MSG_LOG!("{}\tLatency\tLatency changed", src_name);
            }
            MessageView::ClockLost(_) => {
                GPS_MSG_LOG!("{}\tClockLost\tClock lost", src_name);
            }
            MessageView::NewClock(nc) => {
                let clock_name = nc
                    .clock()
                    .map(|c| c.name())
                    .unwrap_or_else(|| glib::GString::from("none"));
                GPS_MSG_LOG!("{}\tNewClock\t{}", src_name, clock_name);
            }
            MessageView::StructureChange(sc) => {
                GPS_MSG_LOG!("{}\tStructureChange\t{:?}", src_name, sc.get());
            }
            MessageView::StreamStart(_) => {
                GPS_MSG_LOG!("{}\tStreamStart\tStream started", src_name);
            }
            MessageView::NeedContext(nc) => {
                GPS_MSG_LOG!("{}\tNeedContext\t{}", src_name, nc.context_type());
            }
            MessageView::HaveContext(hc) => {
                let context = hc.context();
                GPS_MSG_LOG!("{}\tHaveContext\t{}", src_name, context.context_type());
            }
            MessageView::Element(elem) => {
                let structure_name = elem
                    .structure()
                    .map(|s| s.name().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                GPS_MSG_LOG!("{}\tElement\t{}", src_name, structure_name);
            }
            MessageView::SegmentDone(_) => {
                GPS_MSG_LOG!("{}\tSegmentDone\tSegment done", src_name);
            }
            MessageView::Qos(qos) => {
                let (live, running_time, stream_time, timestamp, duration) = qos.get();
                GPS_MSG_LOG!(
                    "{}\tQos\tlive: {}, running time: {:?}, stream time: {:?}, timestamp: {:?}, duration: {:?}",
                    src_name,
                    live,
                    running_time,
                    stream_time,
                    timestamp,
                    duration
                );
            }
            MessageView::Toc(_) => {
                GPS_MSG_LOG!("{}\tToc\tTable of Contents", src_name);
            }
            MessageView::ResetTime(_) => {
                GPS_MSG_LOG!("{}\tResetTime\tReset time", src_name);
            }
            MessageView::StreamCollection(sc) => {
                GPS_MSG_LOG!(
                    "{}\tStreamCollection\t{} stream(s)",
                    src_name,
                    sc.stream_collection().len()
                );
            }
            MessageView::StreamsSelected(ss) => {
                GPS_MSG_LOG!(
                    "{}\tStreamsSelected\t{} stream(s)",
                    src_name,
                    ss.stream_collection().len()
                );
            }
            MessageView::Redirect(red) => {
                let count = red.entries().count();
                GPS_MSG_LOG!("{}\tRedirect\t{} entries", src_name, count);
            }
            MessageView::PropertyNotify(_) => {
                GPS_MSG_LOG!("{}\tPropertyNotify\tProperty changed", src_name);
            }
            MessageView::Application(app_msg) => {
                match app_msg.structure() {
                    // Here we can send ourselves messages from any thread and show them to the user in
                    // the UI in case something goes wrong
                    Some(s) if s.name() == "warning" => {
                        if let Ok(text) = s.get::<&str>("text") {
                            GPS_MSG_LOG!("{}\tApplication\tWarning: {}", src_name, text);
                            GPS_WARN!("{}", text);
                        } else {
                            GPS_MSG_LOG!("{}\tApplication\tWarning (no text)", src_name);
                            GPS_WARN!("Warning message without text");
                        }
                    }
                    Some(s) => {
                        GPS_MSG_LOG!("{}\tApplication\t{}", src_name, s.name());
                    }
                    None => {
                        GPS_MSG_LOG!("{}\tApplication\tApplication message", src_name);
                    }
                }
            }
            _ => {
                // Fallback for unknown message types
                if let Some(structure) = msg.structure() {
                    GPS_MSG_LOG!("{}\tUnknown\t{:?}", src_name, structure);
                } else {
                    GPS_MSG_LOG!("{}\tUnknown\tUnknown message", src_name);
                }
            }
        };
    }

    pub fn pipeline_elements(&self) -> Option<Vec<String>> {
        if self.is_playing() {
            let bin = self
                .pipeline
                .borrow()
                .clone()?
                .dynamic_cast::<gst::Bin>()
                .ok()?;
            Some(
                ElementInfo::search_for_element(&bin, "")
                    .iter()
                    .filter_map(|e| e.factory().map(|f| f.name().to_string()))
                    .collect(),
            )
        } else {
            None
        }
    }

    // Render graph methods
    fn process_gst_node(
        graphview: &GM::GraphView,
        node: &GM::Node,
        elements: &mut HashSet<String>,
        description: &mut String,
    ) {
        let unique_name = node.unique_name();
        let _ = write!(description, "{} name={} ", node.name(), unique_name);
        elements.insert(unique_name.clone());
        // Node properties
        for (name, value) in node.properties().iter() {
            // This allows having an index in front of a property such as an enum.
            if !node.hidden_property(name) {
                // Quote values containing spaces or special chars for GStreamer parser
                // but only if not already quoted
                let needs_quoting =
                    (value.contains(' ') || value.contains('(') || value.contains(')'))
                        && !value.starts_with('"');
                if needs_quoting {
                    let _ = write!(description, "{name}=\"{value}\" ");
                } else {
                    let _ = write!(description, "{name}={value} ");
                }
            }
        }
        // Port properties
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
                    if elements.contains(&node.unique_name()) {
                        let _ = write!(description, "{}. ", node.unique_name());
                    } else {
                        Self::process_gst_node(graphview, &node, elements, description);
                    }
                }
            }
        }
    }

    pub fn pipeline_description_from_graphview(&self, graphview: &GM::GraphView) -> String {
        let source_nodes = graphview.all_nodes(GM::NodeType::Source);
        let mut elements: HashSet<String> = HashSet::new();
        let mut description = String::new();
        for source_node in source_nodes {
            Self::process_gst_node(graphview, &source_node, &mut elements, &mut description);
        }
        description
    }

    pub fn create_links_for_element(&self, element: &gst::Element, graphview: &GM::GraphView) {
        let mut iter = element.iterate_pads();
        let Some(node) = graphview.node_by_unique_name(&element.name()) else {
            GPS_ERROR!("Node not found for element: {}", element.name());
            return;
        };

        loop {
            match iter.next() {
                Ok(Some(pad)) => {
                    GPS_INFO!("Found pad: {}", pad.name());

                    if pad.direction() == gst::PadDirection::Src {
                        let Some(port) = node.port_by_name(&pad.name()) else {
                            GPS_ERROR!("Port not found: {}", pad.name());
                            continue;
                        };
                        if let Some(peer_pad) = pad.peer() {
                            if let Some(peer_element) = peer_pad.parent_element() {
                                let Some(peer_node) =
                                    graphview.node_by_unique_name(&peer_element.name())
                                else {
                                    GPS_ERROR!("Peer node not found: {}", peer_element.name());
                                    continue;
                                };
                                let Some(peer_port) = peer_node.port_by_name(&peer_pad.name())
                                else {
                                    GPS_ERROR!("Peer port not found: {}", peer_pad.name());
                                    continue;
                                };
                                if let Err(e) = self.with_app(|app| {
                                    app.create_link(
                                        node.id(),
                                        peer_node.id(),
                                        port.id(),
                                        peer_port.id(),
                                    );
                                }) {
                                    GPS_ERROR!("Failed to create link: {}", e);
                                }
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
                    let port_direction = if pad.direction() == gst::PadDirection::Src {
                        GM::PortDirection::Output
                    } else {
                        GM::PortDirection::Input
                    };

                    let caps = pad.current_caps().unwrap_or_else(|| pad.query_caps(None));

                    match self.with_app(|app| {
                        app.create_port_with_caps(
                            node.id(),
                            port_direction,
                            GM::PortPresence::Always,
                            caps.to_string(),
                        )
                    }) {
                        Ok(port_id) => {
                            if let Some(port) = node.port(port_id) {
                                port.set_name(&pad_name);
                            }
                        }
                        Err(e) => GPS_ERROR!("Failed to create port: {}", e),
                    }
                }
                Err(gst::IteratorError::Resync) => iter.resync(),
                _ => break,
            }
        }
    }

    /// Check if an element's property value equals the ParamSpec default
    fn is_property_at_default(element: &gst::Element, pspec: &glib::ParamSpec) -> bool {
        let pspec_type = pspec.type_();
        let prop_name = pspec.name();

        // For enum properties, compare via nick strings (more reliable than ParamSpec default)
        if pspec_type.is_a(glib::ParamSpecEnum::static_type()) {
            let current = element.property_value(prop_name);
            match current.get::<&glib::EnumValue>() {
                Ok(enum_val) => {
                    let default_gvalue = pspec.default_value();
                    match default_gvalue.get::<&glib::EnumValue>() {
                        Ok(default_enum) => return enum_val.nick() == default_enum.nick(),
                        Err(_) => {
                            GPS_DEBUG!(
                                "Could not extract default enum value for {}, falling back to string comparison",
                                prop_name
                            );
                            // Fall through to string comparison
                        }
                    }
                }
                Err(_) => {
                    GPS_DEBUG!(
                        "Could not extract current enum value for {}, falling back to string comparison",
                        prop_name
                    );
                    // Fall through to string comparison
                }
            }
        }

        // For flags, compare integer values
        if pspec_type.is_a(glib::ParamSpecFlags::static_type()) {
            if let Ok(flags_pspec) = pspec.clone().downcast::<glib::ParamSpecFlags>() {
                // SAFETY: flags_pspec is a valid GParamSpecFlags obtained via successful downcast.
                // The glib-rs FFI struct layout matches GLib's stable C ABI.
                // default_value is a public field in GParamSpecFlags.
                let default_int = unsafe {
                    let ptr = flags_pspec.as_ptr() as *const glib::gobject_ffi::GParamSpecFlags;
                    (*ptr).default_value
                };
                let current = element.property_value(prop_name);
                if let Ok(flags) = current.get::<Vec<&glib::FlagsValue>>() {
                    let current_int = flags
                        .iter()
                        .copied()
                        .fold(0u32, |acc, val| acc | val.value());
                    return current_int == default_int;
                } else {
                    GPS_DEBUG!(
                        "Could not extract flags value for {}, falling back to string comparison",
                        prop_name
                    );
                }
            }
        }

        // For other types, compare string representations
        // Both element_property() and value_as_str() normalize to lowercase
        if let Ok(value_str) = ElementInfo::element_property(element, prop_name) {
            let default_str = common::value_as_str(pspec.default_value()).unwrap_or_default();
            return value_str == default_str;
        }

        GPS_DEBUG!(
            "Could not compare property {} - assuming non-default to preserve data",
            prop_name
        );
        false
    }

    pub fn create_properties_for_element(&self, element: &gst::Element, node: &GM::Node) {
        let properties = match ElementInfo::element_properties(element) {
            Ok(props) => props,
            Err(e) => {
                GPS_ERROR!("Couldn't get properties for {}: {}", node.name(), e);
                return;
            }
        };
        for (property_name, property_value) in properties {
            let flags = property_value.flags();
            // Skip:
            // - name/parent: internal GStreamer properties
            // - non-readable: can't get value to serialize
            // - non-writable (readonly): can't be set, so no point serializing
            if property_name == "name"
                || property_name == "parent"
                || !flags.contains(glib::ParamFlags::READABLE)
                || !flags.contains(glib::ParamFlags::WRITABLE)
            {
                continue;
            }

            // Get property value string once (avoid fetching twice)
            let value_str = match ElementInfo::element_property(element, &property_name) {
                Ok(s) if !s.is_empty() => s,
                _ => continue,
            };

            // Skip properties that are at their default value
            if Self::is_property_at_default(element, &property_value) {
                continue;
            }

            GPS_DEBUG!(
                "property name {} value_str '{}' (non-default)",
                property_name,
                value_str
            );
            node.add_property(&property_name, &value_str);
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
                        if let Some(factory) = element.factory() {
                            let element_factory_name = factory.name().to_string();
                            let node = graphview.create_node(
                                &element_factory_name,
                                ElementInfo::element_type(&element_factory_name),
                            );
                            node.set_unique_name(&element.name());
                            graphview.add_node(node.clone());
                            self.create_pads_for_element(&element, &node);
                            self.create_properties_for_element(&element, &node);
                            elements.push(element);
                        } else {
                            GPS_WARN!("Element {} has no factory, skipping", element.name());
                        }
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
