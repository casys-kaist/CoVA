use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct SortTracker(ObjectSubclass<imp::SortTracker>) @extends gst_base::BaseTransform, gst::Element, gst::Object;
}

unsafe impl Send for SortTracker {}
unsafe impl Sync for SortTracker {}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "sorttracker",
        gst::Rank::None,
        SortTracker::static_type(),
    )
}
