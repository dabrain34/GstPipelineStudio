use gtk::ffi::GtkDrawingArea;
use gtk::gdk::Display;
use gtk::glib;
use gtk::prelude::*;
use gtk::{
    AboutDialog, AccelFlags, AccelGroup, ApplicationWindow, Builder, DrawingArea, EventBox,
    MenuItem, Viewport, WindowPosition,
};
use std::cell::RefCell;
use std::rc::Rc;

pub fn build_ui(application: &gtk::Application) {
    let glade_src = include_str!("gps.ui");
    let builder = Builder::from_string(glade_src);

    let window: ApplicationWindow = builder.object("mainwindow").expect("Couldn't get window");
    window.set_application(Some(application));

    window.set_title("GstPipelineStudio");
    window.set_position(WindowPosition::Center);
    window.set_size_request(800, 600);

    let drawing_area = DrawingArea::new();
    let view_port: Viewport = builder.object("drawing_area").expect("Couldn't get window");
    let event_box = EventBox::new();
    event_box.add(&drawing_area);
    view_port.add(&event_box);
    let mut position = (0.0, 0.0);
    let position = Rc::new(RefCell::new((0.0, 0.0)));
    let p_clone = position.clone();
    drawing_area.connect_draw(move |w, c| {
        println!("w: {} c:{} p: {:?}", w, c, p_clone);
        c.rectangle(p_clone.borrow().0, p_clone.borrow().1, 10.0, 20.0);
        c.fill();
        gtk::Inhibit(false)
    });

    event_box.connect_button_release_event(move |w, evt| {
        let mut p = position.borrow_mut();
        p.0 = evt.position().0; // = evt.position().clone();
        p.1 = evt.position().1;
        drawing_area.queue_draw();
        gtk::Inhibit(false)
    });
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
