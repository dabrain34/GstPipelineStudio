// resources.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

//! Shared embedded resources for the UI.

use gtk::CssProvider;
use std::sync::Once;

/// Splash banner PNG embedded in the binary (used by splash screen and about dialog)
pub static SPLASH_BANNER_PNG: &[u8] = include_bytes!("../../data/icons/splash-banner.png");

/// Application-wide CSS styles
static APP_CSS: &str = include_str!("app.css");

/// Ensures the app CSS is loaded only once
static CSS_INIT: Once = Once::new();

/// Load the application-wide CSS styles.
/// This function is idempotent and can be called multiple times safely.
pub fn load_app_css() {
    CSS_INIT.call_once(|| {
        let provider = CssProvider::new();
        provider.load_from_data(APP_CSS);

        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}
