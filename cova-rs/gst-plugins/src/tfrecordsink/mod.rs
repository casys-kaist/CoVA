use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct TFRecordSink(ObjectSubclass<imp::TFRecordSink>) @extends gst_base::BaseSink, gst::Element, gst::Object;
}

unsafe impl Send for TFRecordSink {}
unsafe impl Sync for TFRecordSink {}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "tfrecordsink",
        gst::Rank::None,
        TFRecordSink::static_type(),
    )
}
