#![feature(drain_filter)]
use gst::glib;

mod bboxcc;
mod bboxsink;
pub mod cova;
mod metapreprocess;
mod sorttracker;
mod tfrecordsink;
mod utils;

gst::plugin_define!(
    cova,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MIT/X11",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    metapreprocess::register(plugin)?;
    cova::register(plugin)?;
    tfrecordsink::register(plugin)?;
    bboxcc::register(plugin)?;
    bboxsink::register(plugin)?;
    sorttracker::register(plugin)?;
    Ok(())
}
