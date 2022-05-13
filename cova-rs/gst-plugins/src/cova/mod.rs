use gst::glib;
use gst::prelude::*;

mod imp;
mod tracker;

glib::wrapper! {
    pub struct Cova(ObjectSubclass<imp::Cova>) @extends gst::Element, gst::Object;
}

unsafe impl Send for Cova {}
unsafe impl Sync for Cova {}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(Some(plugin), "cova", gst::Rank::None, Cova::static_type())
}
