// settings.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
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

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Settings {
    pub app_maximized: bool,
    pub app_width: i32,
    pub app_height: i32,

    // values must be emitted before tables
    pub favorites: Vec<String>,
    pub paned_positions: HashMap<String, i32>,
    pub preferences: HashMap<String, String>,
}

impl Settings {
    fn settings_file_exist() {
        let s = Settings::get_settings_file_path();

        if !s.exists() {
            if let Some(parent_dir) = s.parent() {
                if !parent_dir.exists() {
                    if let Err(e) = create_dir_all(parent_dir) {
                        GPS_ERROR!(
                            "Error while trying to build settings snapshot_directory '{}': {}",
                            parent_dir.display(),
                            e
                        );
                    }
                }
            }
        }
    }

    fn get_settings_file_path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push(config::APP_ID);
        path.push("settings.toml");
        path
    }
    // Public methods
    pub fn default_graph_file_path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push(config::APP_ID);
        path.push("default_graph.toml");
        path
    }

    pub fn default_log_file_path() -> PathBuf {
        let mut path = PathBuf::new();
        path.push("gstpipelinestudio.log");
        path
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

    pub fn get_favorites_list() -> Vec<String> {
        let mut favorites = Vec::new();
        let settings = Settings::load_settings();
        for fav in settings.favorites {
            favorites.push(fav);
        }
        favorites
    }

    // Save the provided settings to the settings path
    pub fn save_settings(settings: &Settings) {
        Settings::settings_file_exist();
        let s = Settings::get_settings_file_path();
        if let Err(e) = serde_any::to_file(&s, settings) {
            GPS_ERROR!("Error while trying to save file: {} {}", s.display(), e);
        }
    }

    // Load the current settings
    pub fn load_settings() -> Settings {
        let s = Settings::get_settings_file_path();
        if s.exists() && s.is_file() {
            match serde_any::from_file::<Settings, _>(&s) {
                Ok(s) => s,
                Err(e) => {
                    GPS_ERROR!("Error while opening '{}': {}", s.display(), e);
                    Settings::default()
                }
            }
        } else {
            let mut settings = Settings {
                app_width: 800,
                app_height: 600,
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
}
