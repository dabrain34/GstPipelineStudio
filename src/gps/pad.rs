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
use std::str::FromStr;

#[derive(Debug, PartialOrd, PartialEq, Eq)]
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
        }
    }

    pub fn caps(&self) -> Option<&str> {
        self.caps.as_deref()
    }

    pub fn caps_compatible(caps1: &str, caps2: &str) -> bool {
        let Ok(caps1) = gst::Caps::from_str(caps1) else {
            return false;
        };
        let Ok(caps2) = gst::Caps::from_str(caps2) else {
            return false;
        };
        caps1.can_intersect(&caps2)
    }

    pub fn pads(element_name: &str, include_on_request: bool) -> (Vec<PadInfo>, Vec<PadInfo>) {
        let mut input = vec![];
        let mut output = vec![];
        if let Some(feature) = ElementInfo::element_feature(element_name) {
            if let Ok(factory) = feature.downcast::<gst::ElementFactory>() {
                if factory.num_pad_templates() > 0 {
                    let pads = factory.static_pad_templates();
                    for pad in pads {
                        GPS_TRACE!("Found a pad name {}", pad.name_template());
                        if pad.presence() == gst::PadPresence::Always
                            || (include_on_request
                                && matches!(
                                    pad.presence(),
                                    gst::PadPresence::Request | gst::PadPresence::Sometimes
                                ))
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
        }
        (input, output)
    }
}
