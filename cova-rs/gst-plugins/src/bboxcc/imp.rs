use super::process::regionprops;
use bbox::Bbox;
use gst::glib;
use gst::prelude::*;
use gst::{gst_debug, gst_info, gst_trace};
use gst_base::subclass::prelude::*;
use once_cell::sync::Lazy;
use std::i32;
use std::ops::DerefMut;
use std::sync::Mutex;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new("bboxcc", gst::DebugColorFlags::empty(), Some("CoVA plugin"))
});

const DEFAULT_CC_THRESHOLD: u32 = 30;

#[derive(Debug, Clone)]
struct Settings {
    cc_threshold: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            cc_threshold: DEFAULT_CC_THRESHOLD,
        }
    }
}

#[derive(Default)]
struct State {
    video_info: Option<gst_video::VideoInfo>,
}

#[derive(Default)]
pub struct BboxCc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

impl BboxCc {}

#[glib::object_subclass]
impl ObjectSubclass for BboxCc {
    const NAME: &'static str = "BboxCc";
    type Type = super::BboxCc;
    type ParentType = gst_base::BaseTransform;
}

impl ObjectImpl for BboxCc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![glib::ParamSpecUInt::new(
                "cc-threshold",
                "CC Threshold",
                "Threshold for valid connected component",
                0,
                u32::MAX,
                DEFAULT_CC_THRESHOLD,
                glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
            )]
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
            "cc-threshold" => {
                let mut settings = self.settings.lock().unwrap();
                let cc_threshold = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing cc-threshold from {} to {}",
                    settings.cc_threshold,
                    cc_threshold
                );
                settings.cc_threshold = cc_threshold;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "cc-threshold" => {
                let settings = self.settings.lock().unwrap();
                settings.cc_threshold.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for BboxCc {}

impl ElementImpl for BboxCc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "BBox generator with conneted component",
                "Filter/Video",
                "Conneted component algorithm based bounding box generator",
                "Jinwoo Hwang <jwhwang@casys.kaist.ac.kr>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = gst::Caps::new_simple(
                "bbox",
                &[
                    ("width", &gst::IntRange::<i32>::new(0, i32::MAX)),
                    ("height", &gst::IntRange::<i32>::new(0, i32::MAX)),
                ],
            );
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::new_simple(
                "video/x-raw",
                &[
                    ("formats", &gst_video::VideoFormat::Rgba.to_str()),
                    ("width", &gst::IntRange::<i32>::new(0, i32::MAX)),
                    ("height", &gst::IntRange::<i32>::new(0, i32::MAX)),
                ],
            );

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });
        PAD_TEMPLATES.as_ref()
    }
}

impl BaseTransformImpl for BboxCc {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::AlwaysInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = true;

    fn unit_size(&self, _element: &Self::Type, caps: &gst::Caps) -> Option<usize> {
        gst_video::VideoInfo::from_caps(caps)
            .map(|info| info.size())
            .ok()
    }

    fn set_caps(
        &self,
        _element: &Self::Type,
        incaps: &gst::Caps,
        _outcaps: &gst::Caps,
    ) -> Result<(), gst::LoggableError> {
        let in_caps = match gst_video::VideoInfo::from_caps(incaps) {
            Err(_) => return Err(gst::loggable_error!(CAT, "Failed to parse input caps")),
            Ok(info) => info,
        };
        self.state.lock().unwrap().video_info = Some(in_caps);

        Ok(())
    }

    fn stop(&self, element: &Self::Type) -> Result<(), gst::ErrorMessage> {
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
        let other_caps = match direction {
            gst::PadDirection::Sink => caps
                .iter()
                .map(|c| {
                    let mut other = gst::Structure::new("bbox", &[]);
                    if let Ok(width) = c.get::<i32>("width") {
                        let height = c.get::<i32>("height").unwrap();
                        other.set("width", width);
                        other.set("height", height);
                    }
                    other
                })
                .collect(),
            gst::PadDirection::Src => gst::Caps::new_any(),
            _ => panic!("Pad direction unknown"),
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

    fn transform_ip(
        &self,
        element: &Self::Type,
        buf: &mut gst::BufferRef,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.deref_mut();
        let settings = self.settings.lock().unwrap();

        let bboxes = {
            let map = buf.map_readable().unwrap();

            let video_info = state.video_info.as_ref().unwrap();
            let width = video_info.width() as usize;
            let height = video_info.height() as usize;

            regionprops(&map.as_slice(), width, height, settings.cc_threshold as i32).unwrap()
        };

        let serialized = Bbox::serialize_vec(&bboxes);
        let len = serialized.len();
        if buf.maxsize() < len {
            let memory = gst::Memory::with_size(len);
            buf.replace_all_memory(memory);
        } else {
            buf.set_size(len);
        }

        let mut map = buf.map_writable().unwrap();
        map.as_mut_slice().copy_from_slice(&serialized[..]);

        gst_trace!(
            CAT,
            obj: element,
            "{} bounding boxes serialized into {} bytes",
            bboxes.len(),
            len
        );

        Ok(gst::FlowSuccess::Ok)
    }
}
