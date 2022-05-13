use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct MetaPreprocess(ObjectSubclass<imp::MetaPreprocess>) @extends gst_base::BaseTransform, gst::Element, gst::Object;
}

unsafe impl Send for MetaPreprocess {}
unsafe impl Sync for MetaPreprocess {}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "metapreprocess",
        gst::Rank::None,
        MetaPreprocess::static_type(),
    )
}
