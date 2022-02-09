// pad.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::logger;

use crate::gps::ElementInfo;
use crate::graphmanager::{PortDirection, PortPresence};

use gst::prelude::*;

#[derive(Debug, PartialOrd, PartialEq)]
pub struct PadInfo {
    name: Option<String>,
    element_name: Option<String>,
    direction: PortDirection,
    presence: PortPresence,
    caps: Option<String>,
}

impl Default for PadInfo {
    fn default() -> PadInfo {
        PadInfo {
            name: None,
            element_name: None,
            direction: PortDirection::Unknown,
            presence: PortPresence::Unknown,
            caps: None,
        }
    }
}
impl PadInfo {
    pub fn presence(&self) -> PortPresence {
        self.presence
    }

    fn pad_to_port_presence(presence: gst::PadPresence) -> PortPresence {
        match presence {
            gst::PadPresence::Always => PortPresence::Always,
            gst::PadPresence::Sometimes => PortPresence::Sometimes,
            gst::PadPresence::Request => PortPresence::Sometimes,
            _ => PortPresence::Unknown,
        }
    }

    pub fn caps(&self) -> &str {
        self.caps.as_ref().unwrap()
    }

    pub fn pads(element_name: &str, include_on_request: bool) -> (Vec<PadInfo>, Vec<PadInfo>) {
        let feature = ElementInfo::element_feature(element_name).expect("Unable to get feature");
        let mut input = vec![];
        let mut output = vec![];

        if let Ok(factory) = feature.downcast::<gst::ElementFactory>() {
            if factory.num_pad_templates() > 0 {
                let pads = factory.static_pad_templates();
                for pad in pads {
                    GPS_TRACE!("Found a pad name {}", pad.name_template());
                    if pad.presence() == gst::PadPresence::Always
                        || (include_on_request
                            && (pad.presence() == gst::PadPresence::Request
                                || pad.presence() == gst::PadPresence::Sometimes))
                    {
                        if pad.direction() == gst::PadDirection::Src {
                            output.push(PadInfo {
                                name: Some(pad.name_template().to_string()),
                                element_name: Some(element_name.to_string()),
                                direction: PortDirection::Output,
                                presence: PadInfo::pad_to_port_presence(pad.presence()),
                                caps: Some(pad.caps().to_string()),
                            });
                        } else if pad.direction() == gst::PadDirection::Sink {
                            input.push(PadInfo {
                                name: Some(pad.name_template().to_string()),
                                element_name: Some(element_name.to_string()),
                                direction: PortDirection::Input,
                                presence: PadInfo::pad_to_port_presence(pad.presence()),
                                caps: Some(pad.caps().to_string()),
                            });
                        }
                    }
                }
            }
        }
        (input, output)
    }
}
