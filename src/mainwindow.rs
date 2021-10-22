use gtk::glib;
use gtk::prelude::*;
use gtk::{
    AboutDialog, AccelFlags, AccelGroup, ApplicationWindow, Builder, MenuItem, WindowPosition,
};

pub fn build_ui(application: &gtk::Application) {
    let glade_src = include_str!("gps.ui");
    let builder = Builder::from_string(glade_src);

    let window: ApplicationWindow = builder.object("mainwindow").expect("Couldn't get window");
    window.set_application(Some(application));

    window.set_title("GstPipelineStudio");
    window.set_position(WindowPosition::Center);
    window.set_size_request(800, 600);
    window.show_all();

    let quit: MenuItem = builder.object("menu-quit").expect("Couldn't get window");
    let about: MenuItem = builder.object("menu-about").expect("Couldn't get window");

    let accel_group = AccelGroup::new();
    window.add_accel_group(&accel_group);

    quit.connect_activate(glib::clone!(@weak window => move |_| {
        window.close();
    }));

    // `Primary` is `Ctrl` on Windows and Linux, and `command` on macOS
    // It isn't available directly through `gdk::ModifierType`, since it has
    // different values on different platforms.
    let (key, modifier) = gtk::accelerator_parse("<Primary>Q");
    quit.add_accelerator("activate", &accel_group, key, modifier, AccelFlags::VISIBLE);

    about.connect_activate(move |_| {
        let p = AboutDialog::new();
        p.set_authors(&["gtk-rs developers"]);
        p.set_website_label(Some("gtk-rs"));
        p.set_website(Some("http://gtk-rs.org"));
        p.set_authors(&["gtk-rs developers"]);
        p.set_title("About!");
        p.set_transient_for(Some(&window));
        p.show_all();
    });
}
