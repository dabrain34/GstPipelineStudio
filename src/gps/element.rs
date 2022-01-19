// element.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
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

use crate::gps::PadInfo;
use crate::graphmanager::{NodeType, PortDirection, PortPresence};
use crate::logger;
use crate::GPS_INFO;

use gst::glib;
use gst::prelude::*;
use gstreamer as gst;
use std::collections::HashMap;

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

    pub fn element_feature(element_name: &str) -> Option<gst::PluginFeature> {
        let registry = gst::Registry::get();
        gst::Registry::find_feature(&registry, element_name, gst::ElementFactory::static_type())
    }

    pub fn element_description(element_name: &str) -> anyhow::Result<String> {
        let mut desc = String::from("");
        let feature = ElementInfo::element_feature(element_name)
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
                    desc.push_str(&gtk::glib::markup_escape_text(val).to_string());
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
        let feature = ElementInfo::element_feature(element_name).expect("Unable to get feature");

        let factory = feature
            .downcast::<gst::ElementFactory>()
            .expect("Unable to get the factory from the feature");
        let element = factory.create(None)?;
        let params = element.class().list_properties();

        for param in params.iter() {
            GPS_INFO!("Property_name {}", param.name());
            if (param.flags() & glib::ParamFlags::READABLE) == glib::ParamFlags::READABLE
                || (param.flags() & glib::ParamFlags::READWRITE) == glib::ParamFlags::READWRITE
            {
                let value = element.property::<String>(param.name());
                properties_list.insert(String::from(param.name()), value);
            } else if let Some(value) = ElementInfo::value_as_str(param.default_value()) {
                properties_list.insert(String::from(param.name()), value);
            } else {
                GPS_INFO!("Unable to add property_name {}", param.name());
            }
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

    pub fn element_supports_new_pad_request(element_name: &str, direction: PortDirection) -> bool {
        let (inputs, outputs) = PadInfo::pads(element_name, true);
        if direction == PortDirection::Input {
            for input in inputs {
                if input.presence() == PortPresence::Sometimes {
                    return true;
                }
            }
        } else if direction == PortDirection::Output {
            for output in outputs {
                if output.presence() == PortPresence::Sometimes {
                    return true;
                }
            }
        } else {
            GPS_ERROR!("Port direction unknown");
        }
        false
    }
}
