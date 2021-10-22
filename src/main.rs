mod mainwindow;

use gtk::prelude::*;

fn main() {
    //    gio::resources_register_include!("compiled.gresource").unwrap();

    let application = gtk::Application::new(
        Some("com.github.gtk-rs.examples.menu_bar"),
        Default::default(),
    );

    application.connect_activate(mainwindow::build_ui);

    application.run();
}
