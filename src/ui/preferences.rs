// preferences.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::app::settings;
use crate::app::GPSApp;
use crate::logger;
use crate::ui as GPSUI;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

// Layout constants
/// Standard margin for dialog content following GNOME HIG guidelines
const MARGIN_STANDARD: i32 = 12;
/// Compact margin for tighter spacing within components
const MARGIN_COMPACT: i32 = 8;
/// Standard spacing between major UI elements
const SPACING_STANDARD: i32 = 12;
/// Compact spacing for related items within a group
const SPACING_COMPACT: i32 = 8;

// Dialog dimensions
/// Default width of the preferences dialog window
const DIALOG_DEFAULT_WIDTH: i32 = 600;
/// Default height of the preferences dialog window
const DIALOG_DEFAULT_HEIGHT: i32 = 450;
/// Minimum width to ensure dialog remains usable when resized
const DIALOG_MIN_WIDTH: i32 = 450;
/// Minimum height to ensure dialog remains usable when resized
const DIALOG_MIN_HEIGHT: i32 = 400;
/// Minimum height for scrollable content areas to ensure usability
const MIN_CONTENT_HEIGHT: i32 = 300;

// Log level constants
/// Minimum application log level (0 = Error)
const LOG_LEVEL_MIN: f64 = 0.0;
/// Maximum application log level (5 = Trace)
const LOG_LEVEL_MAX: f64 = 5.0;
/// Step increment for log level spinner
const LOG_LEVEL_STEP: f64 = 1.0;

// Search constants
/// Debounce delay in milliseconds for search filtering
const SEARCH_DEBOUNCE_MS: u32 = 300;

// Preference key constants
/// Settings key for GTK4 paintable sink preference
const PREF_KEY_GTK4_SINK: &str = "use_gtk4_sink";
/// Settings key for application log level
const PREF_KEY_LOG_LEVEL: &str = "log_level";
/// Settings key for GStreamer log level
const PREF_KEY_GST_LOG_LEVEL: &str = "gst_log_level";

// UI String constants
/// Placeholder text for the preferences search entry
const STR_SEARCH_PLACEHOLDER: &str = "Search preferences...";
/// Label for the General settings tab
const STR_TAB_GENERAL: &str = "General";
/// Label for the Logging settings tab
const STR_TAB_LOGGING: &str = "Logging";
/// Header for the Video Rendering category
const STR_CATEGORY_VIDEO: &str = "Video Rendering";
/// Header for the Application Logging category
const STR_CATEGORY_APP_LOGGING: &str = "Application Logging";
/// Header for the GStreamer Logging category
const STR_CATEGORY_GST_LOGGING: &str = "GStreamer Logging";
/// Label for GTK4 sink preference
const STR_PREF_GTK4_SINK: &str = "Use GTK4 Paintable Sink";
/// Tooltip for GTK4 sink preference
const STR_TOOLTIP_GTK4_SINK: &str = "Enable gtk4paintablesink element for video rendering";
/// Label for application log level preference
const STR_PREF_APP_LOG_LEVEL: &str = "Application Log Level";
/// Description for application log level preference
const STR_DESC_APP_LOG_LEVEL: &str = "Controls verbosity of application logs (0=Error, 5=Trace)";
/// Label for GStreamer log level preference
const STR_PREF_GST_LOG_LEVEL: &str = "GStreamer Log Level";
/// Description for GStreamer log level preference
const STR_DESC_GST_LOG_LEVEL: &str =
    "GST_DEBUG environment variable format (e.g., *:WARNING,element:DEBUG)";
/// Placeholder for GStreamer log level entry
const STR_PLACEHOLDER_GST_LOG: &str = "e.g., *:WARNING,GST_ELEMENT:DEBUG";
/// Title for the preferences dialog
const STR_DIALOG_TITLE: &str = "Preferences";

fn create_preference_row(
    label_text: &str,
    widget: &gtk::Widget,
    description: Option<&str>,
) -> gtk::ListBoxRow {
    let row_box = gtk::Box::new(gtk::Orientation::Vertical, SPACING_COMPACT);
    row_box.set_margin_start(MARGIN_STANDARD);
    row_box.set_margin_end(MARGIN_STANDARD);
    row_box.set_margin_top(MARGIN_COMPACT);
    row_box.set_margin_bottom(MARGIN_COMPACT);

    // Label and description at the top
    let label = gtk::Label::builder()
        .label(label_text)
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .build();
    row_box.append(&label);

    if let Some(desc) = description {
        let desc_label = gtk::Label::builder()
            .label(desc)
            .halign(gtk::Align::Start)
            .xalign(0.0)
            .css_classes(vec!["dim-label", "caption"])
            .wrap(true)
            .build();
        row_box.append(&desc_label);
    }

    // Widget takes full width below
    widget.set_hexpand(true);
    widget.set_halign(gtk::Align::Fill);
    row_box.append(widget);

    // Create ListBoxRow and set it to fill available space
    let list_row = gtk::ListBoxRow::new();
    list_row.set_child(Some(&row_box));
    list_row.set_activatable(false);

    list_row
}

fn create_settings_category(category_name: &str) -> (gtk::Box, gtk::ListBox) {
    let category_box = gtk::Box::new(gtk::Orientation::Vertical, SPACING_COMPACT);
    category_box.set_margin_start(0);
    category_box.set_margin_end(0);
    category_box.set_margin_top(MARGIN_COMPACT);
    category_box.set_margin_bottom(MARGIN_COMPACT);

    // Category header
    let header = gtk::Label::builder()
        .label(category_name)
        .halign(gtk::Align::Start)
        .margin_start(MARGIN_STANDARD)
        .margin_bottom(MARGIN_COMPACT)
        .css_classes(vec!["heading"])
        .build();
    category_box.append(&header);

    // Settings list box - no horizontal margins, let rows handle it
    let listbox = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(vec!["boxed-list"])
        .build();

    category_box.append(&listbox);

    (category_box, listbox)
}

/// Filters rows in a ListBox based on search text by checking all label content
fn filter_listbox_rows(listbox: &gtk::ListBox, search_text: &str) {
    let mut row_child = listbox.first_child();
    while let Some(row_widget) = row_child {
        if let Some(listbox_row) = row_widget.downcast_ref::<gtk::ListBoxRow>() {
            if let Some(row_box) = listbox_row.child() {
                if let Some(row) = row_box.downcast_ref::<gtk::Box>() {
                    // Collect all label text (title + descriptions) for searching
                    let mut all_text = String::new();
                    let mut child = row.first_child();
                    while let Some(widget) = child {
                        if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                            if !all_text.is_empty() {
                                all_text.push(' ');
                            }
                            all_text.push_str(&label.text().to_lowercase());
                        }
                        child = widget.next_sibling();
                    }

                    listbox_row
                        .set_visible(search_text.is_empty() || all_text.contains(search_text));
                }
            }
        }
        row_child = row_widget.next_sibling();
    }
}

/// Applies search filter to all ListBoxes within a container
fn apply_search_filter(container: &gtk::Box, search_text: &str) {
    let mut child = container.first_child();
    while let Some(widget) = child {
        if let Some(category_box) = widget.downcast_ref::<gtk::Box>() {
            let mut listbox_child = category_box.first_child();
            while let Some(lb_widget) = listbox_child {
                if let Some(listbox) = lb_widget.downcast_ref::<gtk::ListBox>() {
                    filter_listbox_rows(listbox, search_text);
                }
                listbox_child = lb_widget.next_sibling();
            }
        }
        child = widget.next_sibling();
    }
}

/// Creates a preference row with a checkbox widget in horizontal layout
fn create_checkbox_preference_row(
    label_text: &str,
    checkbox: &gtk::CheckButton,
    tooltip: Option<&str>,
) -> gtk::ListBoxRow {
    let row_box = gtk::Box::new(gtk::Orientation::Horizontal, SPACING_STANDARD);
    row_box.set_margin_start(MARGIN_STANDARD);
    row_box.set_margin_end(MARGIN_STANDARD);
    row_box.set_margin_top(MARGIN_COMPACT);
    row_box.set_margin_bottom(MARGIN_COMPACT);

    let label = gtk::Label::builder()
        .label(label_text)
        .halign(gtk::Align::Start)
        .hexpand(true)
        .xalign(0.0)
        .build();

    if let Some(tip) = tooltip {
        label.set_tooltip_text(Some(tip));
    }

    row_box.append(&label);
    row_box.append(checkbox);

    // Create ListBoxRow for consistency with create_preference_row
    let list_row = gtk::ListBoxRow::new();
    list_row.set_child(Some(&row_box));
    list_row.set_activatable(false);

    list_row
}

pub fn display_settings(app: &GPSApp) {
    let settings = settings::Settings::load_settings();

    // Main container
    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Search bar
    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(STR_SEARCH_PLACEHOLDER)
        .margin_start(MARGIN_STANDARD)
        .margin_end(MARGIN_STANDARD)
        .margin_top(MARGIN_STANDARD)
        .margin_bottom(MARGIN_COMPACT)
        .build();

    main_box.append(&search_entry);

    // Create notebook for tabs
    let notebook = gtk::Notebook::new();
    notebook.set_margin_start(0);
    notebook.set_margin_end(0);
    notebook.set_margin_bottom(MARGIN_STANDARD);

    // General settings tab
    let general_scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(MIN_CONTENT_HEIGHT)
        .build();

    let general_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Video Rendering Category
    let (video_category, video_listbox) = create_settings_category(STR_CATEGORY_VIDEO);

    let use_gtk4_sink = gtk::CheckButton::new();
    use_gtk4_sink.set_active(
        settings
            .preferences
            .get(PREF_KEY_GTK4_SINK)
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(true),
    );
    use_gtk4_sink.connect_toggled(glib::clone!(move |c| {
        let mut settings = settings::Settings::load_settings();
        settings
            .preferences
            .insert(PREF_KEY_GTK4_SINK.to_string(), c.is_active().to_string());
        settings::Settings::save_settings(&settings);
    }));

    let video_row = create_checkbox_preference_row(
        STR_PREF_GTK4_SINK,
        &use_gtk4_sink,
        Some(STR_TOOLTIP_GTK4_SINK),
    );
    video_listbox.append(&video_row);

    general_box.append(&video_category);
    general_scrolled.set_child(Some(&general_box));

    // Logging settings tab
    let logging_scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(MIN_CONTENT_HEIGHT)
        .build();

    let logging_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Application Logging Category
    let (app_log_category, app_log_listbox) = create_settings_category(STR_CATEGORY_APP_LOGGING);

    let log_level_spin = gtk::SpinButton::with_range(LOG_LEVEL_MIN, LOG_LEVEL_MAX, LOG_LEVEL_STEP);
    log_level_spin.set_value(
        settings
            .preferences
            .get(PREF_KEY_LOG_LEVEL)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0),
    );
    log_level_spin.connect_value_changed(glib::clone!(move |c| {
        let mut settings = settings::Settings::load_settings();
        settings
            .preferences
            .insert(PREF_KEY_LOG_LEVEL.to_string(), c.value().to_string());
        logger::set_log_level(logger::LogLevel::from_u32(c.value() as u32));
        settings::Settings::save_settings(&settings);
    }));

    let log_level_row = create_preference_row(
        STR_PREF_APP_LOG_LEVEL,
        &log_level_spin.upcast::<gtk::Widget>(),
        Some(STR_DESC_APP_LOG_LEVEL),
    );
    app_log_listbox.append(&log_level_row);

    logging_box.append(&app_log_category);

    // GStreamer Logging Category
    let (gst_log_category, gst_log_listbox) = create_settings_category(STR_CATEGORY_GST_LOGGING);

    let gst_log_entry = gtk::Entry::new();
    gst_log_entry.set_text(settings::Settings::gst_log_level().as_str());
    gst_log_entry.set_placeholder_text(Some(STR_PLACEHOLDER_GST_LOG));
    gst_log_entry.connect_changed(glib::clone!(move |c| {
        let mut settings = settings::Settings::load_settings();
        settings
            .preferences
            .insert(PREF_KEY_GST_LOG_LEVEL.to_string(), c.text().to_string());
        settings::Settings::save_settings(&settings);
    }));

    let gst_log_row = create_preference_row(
        STR_PREF_GST_LOG_LEVEL,
        &gst_log_entry.upcast::<gtk::Widget>(),
        Some(STR_DESC_GST_LOG_LEVEL),
    );
    gst_log_listbox.append(&gst_log_row);

    logging_box.append(&gst_log_category);
    logging_scrolled.set_child(Some(&logging_box));

    // Add tabs to notebook
    notebook.append_page(
        &general_scrolled,
        Some(&gtk::Label::new(Some(STR_TAB_GENERAL))),
    );
    notebook.append_page(
        &logging_scrolled,
        Some(&gtk::Label::new(Some(STR_TAB_LOGGING))),
    );

    main_box.append(&notebook);

    // Search functionality with debouncing
    let general_box_weak = general_box.downgrade();
    let logging_box_weak = logging_box.downgrade();

    // Store the timeout source ID to cancel previous searches
    let timeout_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    search_entry.connect_search_changed(move |entry| {
        let search_text = entry.text().to_lowercase();
        let general_box_weak = general_box_weak.clone();
        let logging_box_weak = logging_box_weak.clone();
        let timeout_id = timeout_id.clone();

        // Cancel any pending search timeout
        if let Some(id) = timeout_id.borrow_mut().take() {
            id.remove();
        }

        // Schedule a new search after debounce delay
        let new_id = glib::timeout_add_local_once(
            std::time::Duration::from_millis(SEARCH_DEBOUNCE_MS as u64),
            move || {
                for box_weak in [&general_box_weak, &logging_box_weak] {
                    if let Some(container) = box_weak.upgrade() {
                        apply_search_filter(&container, &search_text);
                    }
                }
            },
        );

        *timeout_id.borrow_mut() = Some(new_id);
    });

    // Set expansion properties
    main_box.set_hexpand(true);
    main_box.set_vexpand(true);

    let dialog = GPSUI::dialog::create(STR_DIALOG_TITLE, app, &main_box, move |_app, _dialog| {
        // Preferences are saved automatically on change, so Apply button just acknowledges
        // No need to close the dialog - let user close it when done
    });

    // Configure dialog sizing for flexibility across different screen sizes
    dialog.set_default_size(DIALOG_DEFAULT_WIDTH, DIALOG_DEFAULT_HEIGHT);

    // Set minimum content size to ensure usability when resized smaller
    // Using size_request on content ensures the dialog can't be resized too small
    main_box.set_size_request(DIALOG_MIN_WIDTH, DIALOG_MIN_HEIGHT);

    dialog.present();
}
