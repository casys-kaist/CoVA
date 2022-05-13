use gst::glib;
use gst::prelude::*;

mod imp;
mod process;

glib::wrapper! {
    pub struct BboxCc(ObjectSubclass<imp::BboxCc>) @extends gst_base::BaseTransform, gst::Element, gst::Object;
}

unsafe impl Send for BboxCc {}
unsafe impl Sync for BboxCc {}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "bboxcc",
        gst::Rank::None,
        BboxCc::static_type(),
    )
}
