// element.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::gps::PadInfo;
use crate::graphmanager::{NodeType, PortDirection, PortPresence};
use crate::logger;
use crate::GPS_INFO;

use gst::glib;
use gst::prelude::*;
use std::collections::HashMap;
use std::fmt::Write as _;

#[derive(Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ElementInfo {
    pub name: String,
    plugin_name: String,
    rank: i32,
}

impl ElementInfo {
    pub fn elements_list() -> anyhow::Result<Vec<ElementInfo>> {
        let registry = gst::Registry::get();
        let mut elements: Vec<ElementInfo> = Vec::new();
        let plugins = gst::Registry::plugins(&registry);
        for plugin in plugins {
            let plugin_name = gst::Plugin::plugin_name(&plugin);
            let features = gst::Registry::features_by_plugin(&registry, &plugin_name);
            for feature in features {
                let mut element = ElementInfo::default();
                if let Ok(factory) = feature.downcast::<gst::ElementFactory>() {
                    let feature = factory.upcast::<gst::PluginFeature>();

                    element.name = gst::PluginFeature::name(&feature).as_str().to_owned();
                    element.plugin_name = gst::Plugin::plugin_name(&plugin).as_str().to_owned();
                    elements.push(element);
                }
            }
        }
        elements.sort();
        Ok(elements)
    }

    pub fn element_feature(element_name: &str) -> Option<gst::PluginFeature> {
        let registry = gst::Registry::get();
        gst::Registry::find_feature(&registry, element_name, gst::ElementFactory::static_type())
    }

    pub fn element_update_rank(element_name: &str, rank: gst::Rank) {
        let feature: Option<gst::PluginFeature> = ElementInfo::element_feature(element_name);
        if let Some(feature) = feature {
            feature.set_rank(rank);
        }
    }

    pub fn element_description(element_name: &str) -> anyhow::Result<String> {
        let mut desc = String::from("");
        let feature = ElementInfo::element_feature(element_name)
            .ok_or_else(|| glib::bool_error!("Failed get element feature"))?;
        let rank = feature.rank();
        if let Ok(factory) = feature.downcast::<gst::ElementFactory>() {
            desc.push_str("<b>Factory details:</b>\n");
            desc.push_str("<b>Rank:</b>");
            let _ = write!(desc, "{:?}", rank);
            desc.push('\n');
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
                    desc.push_str(&gtk::glib::markup_escape_text(val));
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
                desc.push_str(&gtk::glib::markup_escape_text(&plugin.description()));
                desc.push('\n');
                desc.push_str("<b>Filename:");
                desc.push_str("</b>");
                desc.push_str(&gtk::glib::markup_escape_text(
                    &plugin
                        .filename()
                        .unwrap_or_default()
                        .as_path()
                        .display()
                        .to_string(),
                ));
                desc.push('\n');
                desc.push_str("<b>Version:");
                desc.push_str("</b>");
                desc.push_str(&gtk::glib::markup_escape_text(&plugin.version()));
                desc.push('\n');
            }
        }
        Ok(desc)
    }

    pub fn element_type(element_name: &str) -> NodeType {
        let (inputs, outputs) = PadInfo::pads(element_name, true);
        let mut element_type = NodeType::Source;
        if !inputs.is_empty() {
            if !outputs.is_empty() {
                element_type = NodeType::Transform;
            } else {
                element_type = NodeType::Sink;
            }
        } else if !outputs.is_empty() {
            element_type = NodeType::Source;
        }

        element_type
    }
    pub fn element_property(element_name: &str, property_name: &str) -> anyhow::Result<String> {
        let feature = ElementInfo::element_feature(element_name).expect("Unable to get feature");

        let factory = feature
            .downcast::<gst::ElementFactory>()
            .expect("Unable to get the factory from the feature");
        let element = factory.create(None)?;
        let value = element
            .try_property::<String>(property_name)
            .unwrap_or_default();
        Ok(value)
    }

    pub fn element_properties(
        element_name: &str,
    ) -> anyhow::Result<HashMap<String, glib::ParamSpec>> {
        let mut properties_list = HashMap::new();
        let feature = ElementInfo::element_feature(element_name).expect("Unable to get feature");

        let factory = feature
            .downcast::<gst::ElementFactory>()
            .expect("Unable to get the factory from the feature");
        let element = factory.create(None)?;
        let params = element.class().list_properties();

        for param in params.iter() {
            let value = element
                .try_property::<String>(param.name())
                .unwrap_or_default();
            GPS_INFO!(
                "Property_name {}={} type={:?}",
                param.name(),
                value,
                param.type_()
            );
            properties_list.insert(String::from(param.name()), param.clone());
        }
        Ok(properties_list)
    }

    pub fn element_is_uri_src_handler(element_name: &str) -> bool {
        let feature = ElementInfo::element_feature(element_name).expect("Unable to get feature");

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

    pub fn element_supports_new_pad_request(
        element_name: &str,
        direction: PortDirection,
    ) -> Option<PadInfo> {
        let (inputs, outputs) = PadInfo::pads(element_name, true);
        if direction == PortDirection::Input {
            for input in inputs {
                if input.presence() == PortPresence::Sometimes {
                    return Some(input);
                }
            }
        } else if direction == PortDirection::Output {
            for output in outputs {
                if output.presence() == PortPresence::Sometimes {
                    return Some(output);
                }
            }
        } else {
            GPS_ERROR!("Port direction unknown");
        }
        None
    }

    pub fn search_fo_element(bin: &gst::Bin, element_name: &str) -> Vec<gst::Element> {
        let mut iter = bin.iterate_elements();
        let mut elements: Vec<gst::Element> = Vec::new();
        elements = loop {
            match iter.next() {
                Ok(Some(element)) => {
                    if element.is::<gst::Bin>() {
                        let bin = element.dynamic_cast::<gst::Bin>().unwrap();
                        let mut bin_elements = ElementInfo::search_fo_element(&bin, element_name);
                        elements.append(&mut bin_elements);
                    } else {
                        GPS_INFO!("Found factory: {}", element.factory().unwrap().name());
                        if element.factory().unwrap().name() == element_name
                            || element_name.is_empty()
                        {
                            GPS_INFO!("Found {}", element_name);
                            elements.push(element);
                        }
                    }
                }
                Err(gst::IteratorError::Resync) => iter.resync(),
                _ => break elements,
            }
        };
        elements
    }
}
