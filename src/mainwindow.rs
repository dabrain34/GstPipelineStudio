use gtk::cairo::Context;
use gtk::glib;
use gtk::prelude::*;
use gtk::{
    AboutDialog, AccelFlags, AccelGroup, ApplicationWindow, Builder, DrawingArea, EventBox,
    MenuItem, Viewport, WindowPosition,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Default)]
struct Element {
    name: String,
    position: (f64, f64),
    size: (f64, f64),
}

fn draw_elements(elements: &Vec<Element>, c: &Context) {
    for element in elements {
        c.rectangle(element.position.0, element.position.1, 80.0, 45.0);
        c.fill().expect("Can not draw into context");
    }
}

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
    let elements: Rc<RefCell<Vec<Element>>> = Rc::new(RefCell::new(vec![]));
    let e_clone = elements.clone();
    drawing_area.connect_draw(move |w, c| {
        println!("w: {} c:{} e: {:?}", w, c, e_clone);
        draw_elements(&e_clone.borrow().to_vec(), c);
        gtk::Inhibit(false)
    });

    event_box.connect_button_release_event(move |_w, evt| {
        let mut elements = elements.borrow_mut();
        let mut element: Element = Default::default();
        element.position.0 = evt.position().0;
        element.position.1 = evt.position().1;
        elements.push(element);
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
