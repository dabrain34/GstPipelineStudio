// panels.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

//! Paned widget positioning and layout management.
//!
//! Manages GTK Paned widget positions for split views. In windowed mode, positions
//! are saved/restored from settings. In maximized mode, positions are calculated
//! proportionally (graph gets 3/4 width and height).

use gtk::glib;
use gtk::prelude::*;
use gtk::Paned;

use crate::logger;
use crate::{GPS_DEBUG, GPS_WARN};

use super::super::settings::Settings;
use super::super::GPSApp;

// Constants for paned widget names
pub const PANED_GRAPH_DASHBOARD: &str = "graph_dashboard-paned";
pub const PANED_GRAPH_LOGS: &str = "graph_logs-paned";
pub const PANED_ELEMENTS_PREVIEW: &str = "elements_preview-paned";
pub const PANED_ELEMENTS_PROPERTIES: &str = "elements_properties-paned";

// Constants for default positions and ratios
pub const DEFAULT_PANED_POSITION: i32 = 100;
const PANED_RATIO_GRAPH: i32 = 3; // Graph area gets 4/5
const PANED_RATIO_TOTAL: i32 = 4;
const PANED_RATIO_ELEMENTS: i32 = 3; // Elements get 3/5 of their area
const MIN_PANED_SIZE: i32 = 100;

impl GPSApp {
    pub fn set_paned_position(
        &self,
        settings: &Settings,
        paned_name: &str,
        paned_default_position: i32,
    ) {
        let paned: Paned = self
            .builder
            .object(paned_name)
            .unwrap_or_else(|| panic!("Couldn't get {}", paned_name));
        paned.set_position(
            *settings
                .paned_positions
                .get(paned_name)
                .unwrap_or(&paned_default_position),
        );
    }

    pub fn save_paned_position(&self, settings: &mut Settings, paned_name: &str) {
        let paned: Paned = self
            .builder
            .object(paned_name)
            .unwrap_or_else(|| panic!("Couldn't get {}", paned_name));
        settings
            .paned_positions
            .insert(paned_name.to_string(), paned.position());
    }

    pub fn apply_paned_positions(&self, is_maximized: bool) {
        let graph_dashboard_paned: Paned = self
            .builder
            .object(PANED_GRAPH_DASHBOARD)
            .expect("Couldn't get graph_dashboard-paned");
        let graph_logs_paned: Paned = self
            .builder
            .object(PANED_GRAPH_LOGS)
            .expect("Couldn't get graph_logs-paned");
        let elements_preview_paned: Paned = self
            .builder
            .object(PANED_ELEMENTS_PREVIEW)
            .expect("Couldn't get elements_preview-paned");
        let elements_properties_paned: Paned = self
            .builder
            .object(PANED_ELEMENTS_PROPERTIES)
            .expect("Couldn't get elements_properties-paned");

        // Get the actual allocated dimensions
        let h_allocation = graph_dashboard_paned.allocation();
        let h_width = h_allocation.width();

        let v_allocation = graph_logs_paned.allocation();
        let v_height = v_allocation.height();

        if h_width > MIN_PANED_SIZE && v_height > MIN_PANED_SIZE {
            // Set horizontal split: graph area gets 4/5 of paned width
            let h_position = (h_width * PANED_RATIO_GRAPH) / PANED_RATIO_TOTAL;
            graph_dashboard_paned.set_position(h_position);

            // Set vertical split: graph gets 4/5 of paned height
            let v_position = (v_height * PANED_RATIO_GRAPH) / PANED_RATIO_TOTAL;
            graph_logs_paned.set_position(v_position);

            // Align elements_preview with graph_logs - use same position to align preview with logs
            elements_preview_paned.set_position(v_position);
            GPS_DEBUG!(
                "elements_preview_paned: aligned with logs at position={}",
                v_position
            );

            // Split elements from properties: 3/5 for elements, 2/5 for details
            let elements_properties_position =
                (v_position * PANED_RATIO_ELEMENTS) / PANED_RATIO_TOTAL;
            elements_properties_paned.set_position(elements_properties_position);
            GPS_DEBUG!(
                "elements_properties_paned: position={} (3/5 of v_position={})",
                elements_properties_position,
                v_position
            );

            let mode = if is_maximized {
                "Maximized"
            } else {
                "Windowed mode"
            };
            GPS_DEBUG!(
                "{} - Setting paned positions: h_width={}, v_height={}, h_pos={}, v_pos={}",
                mode,
                h_width,
                v_height,
                h_position,
                v_position
            );
        } else if !is_maximized {
            // Fallback to saved positions if allocation is not ready (only in windowed mode)
            let settings = Settings::load_settings();
            self.set_paned_position(&settings, PANED_GRAPH_DASHBOARD, 600);
            self.set_paned_position(&settings, PANED_GRAPH_LOGS, 400);
            GPS_DEBUG!("Windowed mode - Using saved positions");
        } else {
            GPS_WARN!(
                "Invalid paned sizes: h_width={}, v_height={}",
                h_width,
                v_height
            );
        }
    }
}
