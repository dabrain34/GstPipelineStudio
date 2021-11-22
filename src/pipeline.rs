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
use gst::prelude::*;
use gstreamer as gst;
use std::error;

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

#[derive(Debug)]
pub struct Pipeline {
    initialized: bool,
}

impl Default for Pipeline {
    fn default() -> Pipeline {
        Pipeline { initialized: false }
    }
}

impl Pipeline {
    pub fn new() -> Result<Self, Box<dyn error::Error>> {
        gst::init()?;
        Ok(Self { initialized: true })
    }

    pub fn elements_list() -> Result<Vec<ElementInfo>, Box<dyn error::Error>> {
        let registry = gst::Registry::get();
        let mut elements: Vec<ElementInfo> = Vec::new();
        let plugins = gst::Registry::get_plugin_list(&registry);
        for plugin in plugins {
            let plugin_name = gst::Plugin::get_plugin_name(&plugin);
            let features = gst::Registry::get_feature_list_by_plugin(&registry, &plugin_name);
            for feature in features {
                let mut element = ElementInfo::default();
                if let Ok(factory) = feature.downcast::<gst::ElementFactory>() {
                    let feature = factory.upcast::<gst::PluginFeature>();

                    element.name = Some(gst::PluginFeature::get_name(&feature).as_str().to_owned());
                    element.plugin_name =
                        Some(gst::Plugin::get_plugin_name(&plugin).as_str().to_owned());
                    elements.push(element);
                }
            }
        }
        elements.sort();
        Ok(elements)
    }
    pub fn element_description(
        element_name: &str,
    ) -> anyhow::Result<String, Box<dyn error::Error>> {
        let mut desc = String::from("");
        let registry = gst::Registry::get();
        let feature = gst::Registry::find_feature(
            &registry,
            element_name,
            gst::ElementFactory::static_type(),
        )
        .expect("Unable to find the element name");

        if let Ok(factory) = feature.downcast::<gst::ElementFactory>() {
            desc.push_str("<b>Factory details:</b>\n");
            desc.push_str("<b>Name:</b>");
            desc.push_str(&factory.get_name());
            desc.push_str("\n");

            let element_keys = factory.get_metadata_keys();
            for key in element_keys {
                let val = factory.get_metadata(&key);
                match val {
                    Some(val) => {
                        desc.push_str("<b>");
                        desc.push_str(&key);
                        desc.push_str("</b>:");
                        desc.push_str(&gtk::glib::markup_escape_text(&val).to_string());
                        desc.push_str("\n");
                    }
                    None => {}
                }
            }
            let feature = factory.upcast::<gst::PluginFeature>();
            let plugin = gst::PluginFeature::get_plugin(&feature);
            match plugin {
                Some(plugin) => {
                    desc.push_str("\n");
                    desc.push_str("<b>Plugin details:</b>");
                    desc.push_str("\n");
                    desc.push_str("<b>Name:");
                    desc.push_str("</b>");
                    desc.push_str(gst::Plugin::get_plugin_name(&plugin).as_str());
                    desc.push_str("\n");
                    desc.push_str("<b>Description:");
                    desc.push_str("</b>");
                    desc.push_str(
                        &gtk::glib::markup_escape_text(&plugin.get_description()).to_string(),
                    );
                    desc.push_str("\n");
                    desc.push_str("<b>Filename:");
                    desc.push_str("</b>");
                    desc.push_str(
                        &gtk::glib::markup_escape_text(
                            &plugin
                                .get_filename()
                                .unwrap()
                                .as_path()
                                .display()
                                .to_string(),
                        )
                        .to_string(),
                    );
                    desc.push_str("\n");
                    desc.push_str("<b>Version:");
                    desc.push_str("</b>");
                    desc.push_str(
                        &gtk::glib::markup_escape_text(&plugin.get_version()).to_string(),
                    );
                    desc.push_str("\n");
                }
                None => {}
            }
        }
        Ok(desc)
    }
}
