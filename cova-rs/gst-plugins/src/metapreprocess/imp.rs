use gst::glib;
use gst::prelude::*;
use gst::{gst_debug, gst_info};
use gst_base::subclass::prelude::*;
use std::collections::LinkedList;

use std::ops::DerefMut;
use std::sync::Mutex;

use once_cell::sync::Lazy;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "metapreprocess",
        gst::DebugColorFlags::empty(),
        Some("Metadata preprocessor"),
    )
});

const DEFAULT_TIMESTEP: u32 = 1;
const DEFAULT_GAMMA: u32 = 1;

#[derive(Debug, Clone, Copy)]
struct Settings {
    timestep: u32,
    gamma: u32
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            timestep: DEFAULT_TIMESTEP,
            gamma: DEFAULT_GAMMA,
        }
    }
}

struct State {
    gamma_idx: usize,
    size_per_buf: usize,
    prev_buffers: LinkedList<gst::Buffer>
}

#[derive(Default)]
pub struct MetaPreprocess {
    settings: Mutex<Settings>,
    state: Mutex<Option<State>>,
}

#[glib::object_subclass]
impl ObjectSubclass for MetaPreprocess {
    const NAME: &'static str = "MetaPreprocess";
    type Type = super::MetaPreprocess;
    type ParentType = gst_base::BaseTransform;
}

impl ObjectImpl for MetaPreprocess {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![glib::ParamSpecUInt::new(
                "timestep",
                "Time step",
                "Number of buffers to stack in temporal domain",
                1,
                u32::MAX,
                DEFAULT_TIMESTEP,
                glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_READY,
            ),
            glib::ParamSpecUInt::new(
                "gamma",
                "Value setting how often should frame be passed",
                "Value setting how often should frame be passed",
                1,
                u32::MAX,
                1,
                glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_READY
            )
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(
        &self,
        obj: &Self::Type,
        _id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec,
    ) {
        match pspec.name() {
            "timestep" => {
                let mut settings = self.settings.lock().unwrap();
                let timestep = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing timestep from {} to {}",
                    settings.timestep,
                    timestep
                );
                settings.timestep = timestep;
            },
            "gamma" => {
                let mut settings = self.settings.lock().unwrap();
                let gamma = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing timestep from {} to {}",
                    settings.gamma,
                    gamma
                );
                settings.gamma = gamma;
            },
            _ => unimplemented!(),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "timestep" => {
                let settings = self.settings.lock().unwrap();
                settings.timestep.to_value()
            },
            "gamma" => {
                let settings = self.settings.lock().unwrap();
                settings.gamma.to_value()
            },
            _ => unimplemented!(),
        }
    }
}


impl GstObjectImpl for MetaPreprocess {}

impl ElementImpl for MetaPreprocess {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "Metadata Preprocessor",
                "Filter/Effect/Converter/Video",
                "Preprocess metadatas extracted from avdec",
                "Anonymous CoVA <anonymouscova@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = gst::Caps::builder("video/x-raw")
                .field(
                    "format",
                    gst_video::VideoFormat::Rgba.to_str(),
                )
                .field("width", gst::IntRange::new(0, i32::MAX))
                .field("height", gst::IntRange::new(0, i32::MAX))
                .build();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
                .unwrap();

            let caps = gst::Caps::builder("video/x-raw")
                .field(
                    "format",
                    gst_video::VideoFormat::I420.to_str(),
                )
                .field("width", gst::IntRange::new(0, i32::MAX))
                .field("height", gst::IntRange::new(0, i32::MAX))
                .build();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
                .unwrap();

            vec![src_pad_template, sink_pad_template]
        });
        PAD_TEMPLATES.as_ref()
    }
}

impl BaseTransformImpl for MetaPreprocess {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::NeverInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;

    fn unit_size(&self, _element: &Self::Type, caps: &gst::Caps) -> Option<usize> {
        gst_video::VideoInfo::from_caps(caps).map(|info| info.size()).ok()
    }

    fn set_caps(
        &self,
        element: &Self::Type,
        incaps: &gst::Caps,
        outcaps: &gst::Caps,
    ) -> Result<(), gst::LoggableError> {
        let _in_info = match gst_video::VideoInfo::from_caps(incaps) {
            Err(_) => return Err(gst::loggable_error!(CAT, "Failed to parse input caps")),
            Ok(info) => info,
        };
        let out_info = match gst_video::VideoInfo::from_caps(outcaps) {
            Err(_) => return Err(gst::loggable_error!(CAT, "Failed to parse output caps")),
            Ok(info) => info,
        };

        let timestep = self.settings.lock().unwrap().timestep as usize;
        let size = out_info.size() as usize;

        gst_debug!(
            CAT,
            obj: element,
            "Configured for caps {} to {}",
            incaps,
            outcaps
        );

        *self.state.lock().unwrap() = Some(State {
            gamma_idx: 0,
            prev_buffers: LinkedList::new(),
            size_per_buf: size / timestep
        });
        Ok(())
    }

    fn stop(&self, element: &Self::Type) -> Result<(), gst::ErrorMessage> {
        // Drop state
        let _ = self.state.lock().unwrap().take();

        gst_info!(CAT, obj: element, "Stopped");

        Ok(())
    }

    fn transform_caps(
        &self,
        element: &Self::Type,
        direction: gst::PadDirection,
        caps: &gst::Caps,
        filter: Option<&gst::Caps>,
    ) -> Option<gst::Caps> {
        let mut other_caps = caps.clone();
        if direction == gst::PadDirection::Src {
            for s in other_caps.make_mut().iter_mut() {
                s.set("format", &gst_video::VideoFormat::I420.to_str());
            }
        } else {
            for s in other_caps.make_mut().iter_mut() {
                s.set("format", &gst_video::VideoFormat::Rgba.to_str());
                if let Ok(width) = s.get::<i32>("width") {
                    s.set("width", width / 16);
                }
                if let Ok(height) = s.get::<i32>("height") {
                    let settings = self.settings.lock().unwrap();
                    s.set("height", height / 16 * settings.timestep as i32);
                }
            }
        };

        gst_debug!(
            CAT,
            obj: element,
            "Transformed caps from {} to {} in direction {:?}",
            caps,
            other_caps,
            direction
        );

        if let Some(filter) = filter {
            Some(filter.intersect_with_mode(&other_caps, gst::CapsIntersectMode::First))
        } else {
            Some(other_caps)
        }
    }

    fn transform(
        &self,
        _element: &Self::Type,
        inbuf: &gst::Buffer,
        outbuf: &mut gst::BufferRef,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.deref_mut().as_mut().unwrap();
        let size_per_buf = state.size_per_buf;
        let (timestep, gamma) = {
            let settings = self.settings.lock().unwrap();
            (settings.timestep as usize, settings.gamma as usize)
        };

        if state.prev_buffers.len() < timestep - 1 {
            // The first buffer will always
            state.prev_buffers.push_front(inbuf.copy());
            Ok(gst_base::BASE_TRANSFORM_FLOW_DROPPED)
        } else if state.gamma_idx == 0 {
            let mut outmap = outbuf.map_writable().expect("Failed to map writable buffer");
            let data = outmap.as_mut_slice();

            {
                let cur_map = inbuf.map_readable().expect("Failed to map readable buffer");
                data[..size_per_buf].copy_from_slice(&cur_map.as_slice()[..size_per_buf]);
            }
            let mut idx = size_per_buf;

            for p in state.prev_buffers.iter() {
                let prev_map = p.map_readable().expect("Failed to map readable buffer");
                data[idx..idx+size_per_buf].copy_from_slice(&prev_map.as_slice()[..size_per_buf]);
                idx += size_per_buf;
            }
            state.prev_buffers.push_front(inbuf.copy());
            state.prev_buffers.pop_back();
            state.gamma_idx = gamma - 1;
            Ok(gst::FlowSuccess::Ok)
        } else {
            state.prev_buffers.push_front(inbuf.copy());
            state.prev_buffers.pop_back();
            state.gamma_idx -= 1;
            Ok(gst_base::BASE_TRANSFORM_FLOW_DROPPED)
        }

    }
}
