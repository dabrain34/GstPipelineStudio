use crate::app::GPSApp;
use crate::graph::Element;
use crate::pipeline::ElementInfo;
use gtk::{
    glib::{self, clone},
    prelude::*,
    ResponseType,
};

use gtk::{
    CellRendererText, Label, ListStore, Orientation, TreeView, TreeViewColumn, WindowPosition,
};

fn create_and_fill_model(elements: &Vec<ElementInfo>) -> ListStore {
    // Creation of a model with two rows.
    let model = ListStore::new(&[u32::static_type(), String::static_type()]);

    // Filling up the tree view.
    for (i, entry) in elements.iter().enumerate() {
        model.insert_with_values(
            None,
            &[(0, &(i as u32 + 1)), (1, &entry.name.as_ref().unwrap())],
        );
    }
    model
}

fn append_column(tree: &TreeView, id: i32) {
    let column = TreeViewColumn::new();
    let cell = CellRendererText::new();

    column.pack_start(&cell, true);
    // Association of the view's column with the model's `id` column.
    column.add_attribute(&cell, "text", id);
    tree.append_column(&column);
}

fn create_and_setup_view() -> TreeView {
    // Creating the tree view.
    let tree = TreeView::new();

    tree.set_headers_visible(false);
    // Creating the two columns inside the view.
    append_column(&tree, 0);
    append_column(&tree, 1);
    tree
}

pub fn build_plugin_list(app: &GPSApp, elements: &Vec<ElementInfo>) {
    let dialog = gtk::Dialog::with_buttons(
        Some("Edit Item"),
        Some(&app.window),
        gtk::DialogFlags::MODAL,
        &[("Close", ResponseType::Close)],
    );
    dialog.set_title("Plugin list");
    dialog.set_position(WindowPosition::Center);
    dialog.set_default_size(640, 480);

    // Creating a vertical layout to place both tree view and label in the window.
    let vertical_layout = gtk::Box::new(Orientation::Vertical, 0);

    // Creation of the label.
    let label = Label::new(Some(""));

    let tree = create_and_setup_view();

    let model = create_and_fill_model(elements);
    // Setting the model into the view.
    tree.set_model(Some(&model));

    // Adding the view to the layout.
    vertical_layout.add(&tree);
    // Same goes for the label.
    vertical_layout.add(&label);

    // The closure responds to selection changes by connection to "::cursor-changed" signal,
    // that gets emitted when the cursor moves (focus changes).
    let app_weak = app.downgrade();
    tree.connect_cursor_changed(clone!(@weak dialog => move |tree_view| {
        let app = upgrade_weak!(app_weak);
        let selection = tree_view.selection();
        if let Some((model, iter)) = selection.selected() {
            // Now getting back the values from the row corresponding to the
            // iterator `iter`.
            //
            // The `get_value` method do the conversion between the gtk type and Rust.
            label.set_text(&format!(
                "Hello '{}' from row {}",
                model
                    .value(&iter, 1)
                    .get::<String>()
                    .expect("Treeview selection, column 1"),
                model
                    .value(&iter, 0)
                    .get::<u32>()
                    .expect("Treeview selection, column 0"),
            ));
            let element = Element {
                name: model
                .value(&iter, 1)
                .get::<String>()
                .expect("Treeview selection, column 1"),
                position: (100.0,100.0),
                size: (100.0,100.0),
            };

            let element_name = model
            .value(&iter, 1)
            .get::<String>()
            .expect("Treeview selection, column 1");
            app.add_new_element(element);

            //dialog.close();
            println!("{}", element_name);
        }
    }));

    // Adding the layout to the window.
    let content_area = dialog.content_area();
    let scrolled_window = gtk::ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
    scrolled_window.add(&vertical_layout);
    content_area.add(&scrolled_window);

    dialog.connect_response(|dialog, _| dialog.close());
    dialog.show_all();
}
