use std::fs::create_dir_all;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::common;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Settings {
    pub favorites: Vec<String>,
}

impl Settings {
    fn settings_file_exist() {
        let s = Settings::get_settings_file_path();

        if !s.exists() {
            if let Some(parent_dir) = s.parent() {
                if !parent_dir.exists() {
                    if let Err(e) = create_dir_all(parent_dir) {
                        println!(
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
        path.push(common::APPLICATION_NAME);
        path.push("settings.toml");
        path
    }

    // Public methods
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
            println!("Error while trying to save file: {} {}", s.display(), e);
        }
    }

    // Load the current settings
    pub fn load_settings() -> Settings {
        let s = Settings::get_settings_file_path();
        if s.exists() && s.is_file() {
            match serde_any::from_file::<Settings, _>(&s) {
                Ok(s) => s,
                Err(e) => {
                    println!("Error while opening '{}': {}", s.display(), e);
                    Settings::default()
                }
            }
        } else {
            Settings::default()
        }
    }
}
