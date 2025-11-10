// models.rs
//
// Copyright 2024 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::RefCell;

// LogEntry GObject for logger displays
mod imp_log_entry {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::LogEntry)]
    pub struct LogEntry {
        #[property(get, set)]
        time: RefCell<String>,
        #[property(get, set)]
        level: RefCell<String>,
        #[property(get, set)]
        category: RefCell<String>,
        #[property(get, set)]
        file: RefCell<String>,
        #[property(get, set)]
        log: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LogEntry {
        const NAME: &'static str = "GPSLogEntry";
        type Type = super::LogEntry;
    }

    #[glib::derived_properties]
    impl ObjectImpl for LogEntry {}
}

glib::wrapper! {
    pub struct LogEntry(ObjectSubclass<imp_log_entry::LogEntry>);
}

impl LogEntry {
    pub fn new(time: &str, level: &str, category: &str, file: &str, log: &str) -> Self {
        glib::Object::builder()
            .property("time", time)
            .property("level", level)
            .property("category", category)
            .property("file", file)
            .property("log", log)
            .build()
    }

    pub fn new_simple(time: &str, level: &str, log: &str) -> Self {
        Self::new(time, level, "", "", log)
    }
}

// ElementInfo GObject for element browser
mod imp_element_info {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::ElementInfoObject)]
    pub struct ElementInfoObject {
        #[property(get, set)]
        name: RefCell<String>,
        #[property(get, set)]
        plugin: RefCell<String>,
        #[property(get, set)]
        rank: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ElementInfoObject {
        const NAME: &'static str = "GPSElementInfo";
        type Type = super::ElementInfoObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for ElementInfoObject {}
}

glib::wrapper! {
    pub struct ElementInfoObject(ObjectSubclass<imp_element_info::ElementInfoObject>);
}

impl ElementInfoObject {
    pub fn new(name: &str, plugin: &str, rank: &str) -> Self {
        glib::Object::builder()
            .property("name", name)
            .property("plugin", plugin)
            .property("rank", rank)
            .build()
    }
}
