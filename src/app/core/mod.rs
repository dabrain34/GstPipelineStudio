// core/mod.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

//! Core application functionality modules.
//!
//! Organizes GPSApp implementation into focused modules for actions, UI bootstrap,
//! element management, graph tabs, context menus, and panel layout.

// Core GPSApp implementation modules
pub mod actions;
pub mod bootstrap;
pub mod elements;
pub mod graphbook;
pub mod menu;
pub mod panels;
