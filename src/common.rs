// common.rs
//
// Copyright 2021 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
use gtk::glib;

pub fn init() -> Result<()> {
    std::env::set_var("GST_XINITTHREADS", "1");
    gst::init()?;
    #[cfg(feature = "gtk4-plugin")]
    {
        gstgtk4::plugin_register_static().expect("Failed to register gstgtk4 plugin");
    }
    gtk::init()?;
    Ok(())
}

pub fn value_as_str(v: &glib::Value) -> Option<String> {
    match v.type_() {
        glib::Type::I8 => Some(str_some_value!(v, i8).to_string()),
        glib::Type::U8 => Some(str_some_value!(v, u8).to_string()),
        glib::Type::BOOL => Some(str_some_value!(v, bool).to_string()),
        glib::Type::I32 => Some(str_some_value!(v, i32).to_string()),
        glib::Type::U32 => Some(str_some_value!(v, u32).to_string()),
        glib::Type::I64 => Some(str_some_value!(v, i64).to_string()),
        glib::Type::U64 => Some(str_some_value!(v, u64).to_string()),
        glib::Type::F32 => Some(str_some_value!(v, f32).to_string()),
        glib::Type::F64 => Some(str_some_value!(v, f64).to_string()),
        glib::Type::STRING => str_opt_value!(v, String),
        _ => None,
    }
}
