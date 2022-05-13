use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct BboxSink(ObjectSubclass<imp::BboxSink>) @extends gst_base::BaseSink, gst::Element, gst::Object;
}

unsafe impl Send for BboxSink {}
unsafe impl Sync for BboxSink {}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "bboxsink",
        gst::Rank::None,
        BboxSink::static_type(),
    )
}
