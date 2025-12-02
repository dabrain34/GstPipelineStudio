// common.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use gtk::prelude::*;
use gtk::{ColumnViewColumn, SignalListItemFactory};

/// Helper function to create a column for ColumnView with optional fixed width
///
/// # Arguments
/// * `title` - The column title to display
/// * `property` - The property name to bind to the LogEntry or model object
/// * `fixed_width` - Optional fixed width in pixels. If None, the column will be expandable
///
/// # Returns
/// A configured ColumnViewColumn that is resizable and has ellipsization enabled
pub fn create_column_view_column_with_width(
    title: &str,
    property: &str,
    fixed_width: Option<i32>,
) -> ColumnViewColumn {
    let factory = SignalListItemFactory::new();
    let property_name = property.to_string();
    let property_name_clone = property_name.clone();

    factory.connect_setup(move |_, list_item| {
        let label = gtk::Label::new(None);
        label.set_halign(gtk::Align::Start);
        label.set_margin_start(4);
        label.set_margin_end(4);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        list_item
            .downcast_ref::<gtk::ListItem>()
            .expect("Needs to be ListItem")
            .set_child(Some(&label));
    });

    factory.connect_bind(move |_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk::ListItem>()
            .expect("Needs to be ListItem");
        let item = list_item.item().expect("ListItem must have an item");
        let label = list_item
            .child()
            .and_downcast::<gtk::Label>()
            .expect("The child has to be a Label");

        // Get the property value from the item
        let text = item.property::<String>(&property_name_clone);
        label.set_text(&text);
    });

    let column = ColumnViewColumn::new(Some(title), Some(factory));

    // Make column resizable
    column.set_resizable(true);

    // Set fixed width if specified, otherwise expand for "log" column
    if let Some(width) = fixed_width {
        column.set_fixed_width(width);
    } else if &property_name == "log" {
        column.set_expand(true);
    }

    column
}

/// Convenience wrapper for creating an expandable column (no fixed width)
pub fn create_column_view_column(title: &str, property: &str) -> ColumnViewColumn {
    create_column_view_column_with_width(title, property, None)
}
