use anyhow::Result;
use gstreamer as gst;

pub fn init() -> Result<()> {
    unsafe {
        x11::xlib::XInitThreads();
    }
    gst::init()?;
    gtk::init()?;
    Ok(())
}
