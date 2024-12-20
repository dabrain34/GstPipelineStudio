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

    pub fn element_factory_exists(element_name: &str) -> bool {
        match ElementInfo::element_feature(element_name) {
            Some(_feature) => {
                GPS_DEBUG!("Found element factory name {}", element_name);
                true
            }
            None => {
                GPS_ERROR!("Unable to find element factory name {}", element_name);
                false
            }
        }
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
        if !ElementInfo::element_factory_exists(element_name) {
            desc.push_str("<b>Factory details:</b>\n");
            desc.push_str("<b>Name:</b>");
            desc.push_str(element_name);
            desc.push('\n');
            desc.push('\n');
            desc.push_str("Factory unavailable.");
        } else {
            let feature = ElementInfo::element_feature(element_name)
                .ok_or_else(|| glib::bool_error!("Failed get element feature"))?;
            let rank = feature.rank();
            if let Ok(factory) = feature.downcast::<gst::ElementFactory>() {
                desc.push_str("<b>Factory details:</b>\n");
                desc.push_str("<b>Rank:</b>");
                let _ = write!(desc, "{rank:?}",);
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

    pub fn element_property(element: &gst::Element, property_name: &str) -> anyhow::Result<String> {
        let value = element.property_value(property_name);
        if value.type_().is_a(glib::Type::ENUM) {
            let value = value.get::<&glib::EnumValue>().unwrap().nick().to_string();
            Ok(value)
        } else if value.type_().is_a(glib::Type::FLAGS) {
            let value = value.get::<Vec<&glib::FlagsValue>>().unwrap();
            let flags = value.iter().copied().fold(0, |acc, val| acc | val.value());
            Ok(flags.to_string())
        } else if value.type_().is_a(glib::Type::F64) || value.type_().is_a(glib::Type::F32) {
            let value = value
                .transform::<String>()
                .expect("Unable to transform to string")
                .get::<String>()
                .unwrap()
                .replace(',', ".");
            Ok(value)
        } else {
            let value = value
                .transform::<String>()
                .expect("Unable to transform to string")
                .get::<String>()
                .unwrap_or_default()
                .to_lowercase();
            Ok(value)
        }
    }

    pub fn element_property_by_feature_name(
        element_name: &str,
        property_name: &str,
    ) -> anyhow::Result<String> {
        let feature = ElementInfo::element_feature(element_name).expect("Unable to get feature");
        let factory = feature
            .downcast::<gst::ElementFactory>()
            .expect("Unable to get the factory from the feature");
        let element = factory.create().build()?;
        ElementInfo::element_property(&element, property_name)
    }

    pub fn element_properties(
        element: &gst::Element,
    ) -> anyhow::Result<HashMap<String, glib::ParamSpec>> {
        let mut properties_list = HashMap::new();
        let params = element.list_properties();

        for param in params.iter() {
            GPS_INFO!("Property_name {}", param.name());
            if param.flags().contains(glib::ParamFlags::READABLE) {
                match element.property_value(param.name()).transform::<String>() {
                    Ok(value) => {
                        GPS_INFO!(
                            "Property_name {}={} type={:?}",
                            param.name(),
                            value.get::<String>().unwrap_or_default(),
                            param.type_()
                        );
                        properties_list.insert(String::from(param.name()), param.clone());
                    }
                    Err(_e) => {
                        GPS_ERROR!("Unable to convert the param {} to string ", param.name())
                    }
                }
            } else {
                GPS_ERROR!("The param {} is not readable", param.name())
            }
        }
        Ok(properties_list)
    }

    pub fn element_properties_by_feature_name(
        element_name: &str,
    ) -> anyhow::Result<HashMap<String, glib::ParamSpec>> {
        let feature = ElementInfo::element_feature(element_name).expect("Unable to get feature");

        let factory = feature
            .downcast::<gst::ElementFactory>()
            .expect("Unable to get the factory from the feature");
        let element = factory.create().build()?;
        ElementInfo::element_properties(&element)
    }

    pub fn element_has_property(element: &gst::Element, property_name: &str) -> bool {
        let properties = ElementInfo::element_properties(element)
            .unwrap_or_else(|_| panic!("Couldn't get properties for {}", element.name()));

        properties.keys().any(|name| name == property_name)
    }

    pub fn element_is_uri_src_handler(element_name: &str) -> Option<(String, bool)> {
        let feature: gst::PluginFeature =
            ElementInfo::element_feature(element_name).expect("Unable to get feature");
        let mut file_chooser = false;
        let factory = feature
            .downcast::<gst::ElementFactory>()
            .expect("Unable to get the factory from the feature");
        let element = factory
            .create()
            .build()
            .expect("Unable to create an element from the feature");
        if let Ok(uri_handler) = element.clone().dynamic_cast::<gst::URIHandler>() {
            let search_strings = ["file", "pushfile"];
            file_chooser = search_strings
                .iter()
                .any(|s| uri_handler.protocols().contains(&glib::GString::from(*s)));
        }

        if element.is::<gst::Bin>() || ElementInfo::element_type(element_name) == NodeType::Source {
            if ElementInfo::element_has_property(&element, "uri") {
                return Some((String::from("uri"), file_chooser));
            }
            if ElementInfo::element_has_property(&element, "location") {
                return Some((String::from("location"), file_chooser));
            }
        }

        None
    }

    pub fn element_is_uri_sink_handler(element_name: &str) -> Option<(String, bool)> {
        let feature = ElementInfo::element_feature(element_name).expect("Unable to get feature");
        let mut file_chooser = false;
        let factory = feature
            .downcast::<gst::ElementFactory>()
            .expect("Unable to get the factory from the feature");
        let element = factory
            .create()
            .build()
            .expect("Unable to create an element from the feature");

        if let Ok(uri_handler) = element.clone().dynamic_cast::<gst::URIHandler>() {
            file_chooser = uri_handler
                .protocols()
                .contains(&glib::GString::from("file"))
        }

        if ElementInfo::element_type(element_name) == NodeType::Sink {
            if ElementInfo::element_has_property(&element, "uri") {
                return Some((String::from("uri"), file_chooser));
            }
            if ElementInfo::element_has_property(&element, "location") {
                return Some((String::from("location"), file_chooser));
            }
        }

        None
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

    pub fn search_for_element(bin: &gst::Bin, element_name: &str) -> Vec<gst::Element> {
        let mut iter = bin.iterate_elements();
        let mut elements: Vec<gst::Element> = Vec::new();
        elements = loop {
            match iter.next() {
                Ok(Some(element)) => {
                    if element.is::<gst::Bin>() {
                        let bin = element.dynamic_cast::<gst::Bin>().unwrap();
                        let mut bin_elements = ElementInfo::search_for_element(&bin, element_name);
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
