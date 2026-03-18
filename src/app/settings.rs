// settings.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use gtk::glib;
use std::collections::HashMap;
use std::fs::create_dir_all;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config;
use crate::logger;
use crate::{GPS_ERROR, GPS_INFO, GPS_WARN};

fn default_ws_desc() -> String {
    String::from("ws://127.0.0.1:8444")
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub app_maximized: bool,
    pub app_width: i32,
    pub app_height: i32,
    pub recent_pipeline: String,
    pub dark_theme: bool,
    pub clean_shutdown: bool,
    pub session_count: u32,
    #[serde(default = "default_ws_desc")]
    pub ws_desc: String,

    // values must be emitted before tables
    pub favorites: Vec<String>,
    pub recent_open_files: Vec<String>,
    pub paned_positions: HashMap<String, i32>,
    pub preferences: HashMap<String, String>,
}

impl Settings {
    fn create_path_if_not(s: &PathBuf) {
        if !s.exists() {
            if let Err(e) = create_dir_all(s) {
                GPS_ERROR!(
                    "Error while trying to build settings snapshot_directory '{}': {}",
                    s.display(),
                    e
                );
            }
        }
    }

    fn default_app_folder() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push(config::APP_ID);
        if !path.exists() {
            Settings::migrate_legacy_config(&path);
        }
        path
    }

    fn migrate_legacy_config(new_path: &std::path::Path) {
        let mut legacy_path = glib::user_config_dir();
        legacy_path.push(config::LEGACY_APP_ID);
        if legacy_path.exists() && legacy_path.is_dir() {
            GPS_INFO!(
                "Migrating config from '{}' to '{}'",
                legacy_path.display(),
                new_path.display()
            );
            if let Err(e) = std::fs::rename(&legacy_path, new_path) {
                GPS_WARN!("Rename failed ({}), attempting copy instead", e);
                if let Err(e) = Settings::copy_dir(&legacy_path, new_path) {
                    GPS_ERROR!(
                        "Failed to migrate config from '{}': {}",
                        legacy_path.display(),
                        e
                    );
                    // Clean up partial copy so migration retries on next launch
                    let _ = std::fs::remove_dir_all(new_path);
                } else {
                    // Remove legacy directory after successful copy
                    let _ = std::fs::remove_dir_all(&legacy_path);
                }
            }
        }
    }

    fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
        create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let dest_path = dst.join(entry.file_name());
            if file_type.is_dir() {
                Settings::copy_dir(&entry.path(), &dest_path)?;
            } else if file_type.is_file() {
                std::fs::copy(entry.path(), dest_path)?;
            }
        }
        Ok(())
    }

    fn settings_file_path() -> PathBuf {
        let mut path = Settings::default_app_folder();
        Settings::create_path_if_not(&path);
        path.push("settings.toml");
        path
    }
    // Public methods
    pub fn graph_file_path() -> PathBuf {
        let mut path = Settings::default_app_folder();
        Settings::create_path_if_not(&path);
        path.push("default_graph.toml");
        path
    }

    pub fn log_file_path() -> PathBuf {
        let mut path = Settings::default_app_folder();
        Settings::create_path_if_not(&path);
        path.push("gstpipelinestudio.log");
        path
    }

    pub fn gst_log_level() -> String {
        let settings = Settings::load_settings();
        let binding = "0".to_string();
        let level = settings
            .preferences
            .get("gst_log_level")
            .unwrap_or(&binding);
        level.clone()
    }

    /// Check if crash recovery dialog is enabled (default: true)
    pub fn crash_recovery_enabled() -> bool {
        let settings = Settings::load_settings();
        settings
            .preferences
            .get("crash_recovery_enabled")
            .map(|v| v != "false")
            .unwrap_or(true) // Enabled by default
    }

    /// Set whether crash recovery dialog is enabled
    pub fn set_crash_recovery_enabled(enabled: bool) {
        let mut settings = Settings::load_settings();
        settings
            .preferences
            .insert("crash_recovery_enabled".to_string(), enabled.to_string());
        Settings::save_settings(&settings);
    }

    pub fn set_recent_pipeline_description(pipeline: &str) {
        let mut settings = Settings::load_settings();
        settings.recent_pipeline = pipeline.to_string();
        Settings::save_settings(&settings);
    }

    pub fn recent_pipeline_description() -> String {
        let settings = Settings::load_settings();
        settings.recent_pipeline
    }

    pub fn set_dark_theme(dark: bool) {
        let mut settings = Settings::load_settings();
        settings.dark_theme = dark;
        Settings::save_settings(&settings);
    }

    pub fn dark_theme() -> bool {
        let settings = Settings::load_settings();
        settings.dark_theme
    }

    pub fn websocket_description() -> String {
        let settings = Settings::load_settings();
        settings.ws_desc
    }

    pub fn set_websocket_description(ws_desc: &str) {
        let mut settings = Settings::load_settings();
        settings.ws_desc = ws_desc.to_string();
        Settings::save_settings(&settings);
    }

    pub fn add_favorite(favorite: &str) {
        let mut settings = Settings::load_settings();
        settings.favorites.sort();
        settings.favorites.push(String::from(favorite));
        Settings::save_settings(&settings);
    }

    pub fn remove_favorite(favorite: &str) {
        let mut settings = Settings::load_settings();
        settings.favorites.retain(|x| x != favorite);
        Settings::save_settings(&settings);
    }

    pub fn favorites_list() -> Vec<String> {
        let mut favorites = Vec::new();
        let settings = Settings::load_settings();
        for fav in settings.favorites {
            favorites.push(fav);
        }
        favorites
    }

    pub fn add_recent_open_file(filename: &str) {
        let mut settings = Settings::load_settings();
        // Remove the file if it already exists in the list
        settings.recent_open_files.retain(|x| x != filename);
        // Add to the front of the list
        settings.recent_open_files.insert(0, String::from(filename));
        // Keep only the 4 most recent files
        if settings.recent_open_files.len() > 4 {
            settings.recent_open_files.truncate(4);
        }
        Settings::save_settings(&settings);
    }

    pub fn get_recent_open_files() -> Vec<String> {
        let mut recent_open_files = Vec::new();
        let settings = Settings::load_settings();
        for recent in settings.recent_open_files {
            recent_open_files.push(recent);
        }
        recent_open_files
    }

    // Save the provided settings to the settings path
    pub fn save_settings(settings: &Settings) {
        let s = Settings::settings_file_path();
        match toml::to_string_pretty(settings) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&s, content) {
                    GPS_ERROR!("Error while trying to save file: {} {}", s.display(), e);
                }
            }
            Err(e) => {
                GPS_ERROR!("Error while serializing settings: {}", e);
            }
        }
    }

    // Load the current settings
    pub fn load_settings() -> Settings {
        let s = Settings::settings_file_path();
        if s.exists() && s.is_file() {
            match std::fs::read_to_string(&s) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(settings) => settings,
                    Err(e) => {
                        GPS_ERROR!("Error while parsing '{}': {}", s.display(), e);
                        Settings::default()
                    }
                },
                Err(e) => {
                    GPS_ERROR!("Error while opening '{}': {}", s.display(), e);
                    Settings::default()
                }
            }
        } else {
            let mut settings = Settings {
                app_maximized: true,
                app_width: 800,
                app_height: 600,
                ws_desc: String::from("ws://127.0.0.1:8444"),
                ..Default::default()
            };
            settings
                .paned_positions
                .insert(String::from("graph_dashboard-paned"), 600);
            settings
                .paned_positions
                .insert(String::from("graph_logs-paned"), 400);
            settings
                .paned_positions
                .insert(String::from("elements_preview-paned"), 300);
            settings
                .paned_positions
                .insert(String::from("elements_properties-paned"), 150);
            settings
                .paned_positions
                .insert(String::from("playcontrols_position-paned"), 400);
            settings
        }
    }

    /// Mark that the application is starting (clear clean_shutdown flag).
    /// This should be called early in startup after checking needs_crash_recovery().
    pub fn mark_session_start() {
        let mut settings = Settings::load_settings();
        settings.clean_shutdown = false;
        settings.session_count += 1;
        Settings::save_settings(&settings);
    }

    /// Mark that the application is shutting down cleanly.
    /// This should be called in the shutdown handler before saving settings.
    pub fn mark_clean_shutdown(settings: &mut Settings) {
        settings.clean_shutdown = true;
    }

    /// Check if the previous session crashed (did not shut down cleanly).
    /// Returns false on first run, if crash recovery is disabled, or if last session was clean.
    pub fn needs_crash_recovery() -> bool {
        let settings = Settings::load_settings();
        // Check if crash recovery is enabled
        let enabled = settings
            .preferences
            .get("crash_recovery_enabled")
            .map(|v| v != "false")
            .unwrap_or(true);
        // No crash if disabled, first session, or last session was clean
        enabled && settings.session_count > 0 && !settings.clean_shutdown
    }
}
