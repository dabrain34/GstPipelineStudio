// common.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
use gtk::glib;

/// Initialize GTK only. Call this first to enable showing splash screen.
pub fn init_gtk() -> Result<()> {
    gtk::init()?;
    Ok(())
}

/// Initialize GStreamer. This can be slow as it scans the registry.
/// Should be called after showing the splash screen.
pub fn init_gst() -> Result<()> {
    std::env::set_var("GST_XINITTHREADS", "1");
    gst::init()?;
    #[cfg(feature = "gtk4-plugin")]
    {
        gstgtk4::plugin_register_static().expect("Failed to register gstgtk4 plugin");
    }
    Ok(())
}

pub fn value_as_str(v: &glib::Value) -> Option<String> {
    let t = v.type_();
    if t.is_a(glib::Type::ENUM) {
        // Try to get enum nick directly. This works for element property values.
        // For ParamSpec defaults, this may fail - caller should use
        // Player::is_property_at_default() which handles enum comparison specially.
        if let Ok(enum_val) = v.get::<&glib::EnumValue>() {
            return Some(enum_val.nick().to_string());
        }
        // Fallback: try transform to string (may return enum nick or integer)
        return v
            .transform::<String>()
            .ok()
            .and_then(|s| s.get::<String>().ok());
    }
    if t.is_a(glib::Type::FLAGS) {
        return v.get::<Vec<&glib::FlagsValue>>().ok().map(|flags| {
            flags
                .iter()
                .copied()
                .fold(0u32, |acc, val| acc | val.value())
                .to_string()
        });
    }
    // For floats, use transform to match ElementInfo::element_property behavior
    if t.is_a(glib::Type::F64) || t.is_a(glib::Type::F32) {
        return v
            .transform::<String>()
            .ok()
            .and_then(|s| s.get::<String>().ok())
            .map(|s| s.replace(',', "."));
    }
    match t {
        glib::Type::I8 => Some(str_some_value!(v, i8).to_string()),
        glib::Type::U8 => Some(str_some_value!(v, u8).to_string()),
        glib::Type::BOOL => Some(str_some_value!(v, bool).to_string()),
        glib::Type::I32 => Some(str_some_value!(v, i32).to_string()),
        glib::Type::U32 => Some(str_some_value!(v, u32).to_string()),
        glib::Type::I64 => Some(str_some_value!(v, i64).to_string()),
        glib::Type::U64 => Some(str_some_value!(v, u64).to_string()),
        glib::Type::STRING => str_opt_value!(v, String).map(|s| s.to_lowercase()),
        // Fallback for other types (e.g., GstCaps): try transform to string
        // Match element_property() behavior with lowercase normalization
        _ => v
            .transform::<String>()
            .ok()
            .and_then(|s| s.get::<String>().ok())
            .map(|s| s.to_lowercase()),
    }
}
