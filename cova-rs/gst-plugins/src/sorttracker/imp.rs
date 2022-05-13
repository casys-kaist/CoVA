use bbox::Bbox;
use gst::glib;
use gst::prelude::*;
use gst::{gst_debug, gst_info, gst_trace};
use gst_base::subclass::prelude::*;
use once_cell::sync::Lazy;
use sort::Sort;
use std::sync::Mutex;

const DEFAULT_IOU: f32 = 0.1;
const DEFAULT_MAXAGE: u32 = 30;
const DEFAULT_MINHITS: u32 = 30;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "sorttracker",
        gst::DebugColorFlags::empty(),
        Some("SORT based tracker"),
    )
});

#[derive(Debug, Clone, Copy)]
struct Settings {
    maxage: u32,
    minhits: u32,
    iou_threshold: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            maxage: DEFAULT_MAXAGE,
            minhits: DEFAULT_MINHITS,
            iou_threshold: DEFAULT_IOU,
        }
    }
}

#[derive(Default)]
pub struct SortTracker {
    settings: Mutex<Settings>,
    sort: Mutex<Option<Sort>>,
}

#[glib::object_subclass]
impl ObjectSubclass for SortTracker {
    const NAME: &'static str = "SortTracker";
    type Type = super::SortTracker;
    type ParentType = gst_base::BaseTransform;
}

impl ObjectImpl for SortTracker {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpecFloat::new(
                    "iou-threshold",
                    "SORT IoU",
                    "IoU threshold used by SORT",
                    0.,
                    1.,
                    DEFAULT_IOU,
                    glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecUInt::new(
                    "maxage",
                    "SORT Max Age",
                    "Max age parameter used by SORT",
                    0,
                    u32::MAX,
                    DEFAULT_MAXAGE,
                    glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecUInt::new(
                    "minhits",
                    "Track Min Hits",
                    "Min hits parameter used by SORT",
                    0,
                    u32::MAX,
                    DEFAULT_MINHITS,
                    glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
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
            "iou-threshold" => {
                let mut settings = self.settings.lock().unwrap();
                let iou = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing sort-iou from {} to {}",
                    settings.iou_threshold,
                    iou
                );
                settings.iou_threshold = iou;
            }
            "maxage" => {
                let mut settings = self.settings.lock().unwrap();
                let maxage = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing sort-maxage from {} to {}",
                    settings.maxage,
                    maxage
                );
                settings.maxage = maxage;
            }
            "minhits" => {
                let mut settings = self.settings.lock().unwrap();
                let minhits = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing sort-minhits from {} to {}",
                    settings.minhits,
                    minhits
                );
                settings.minhits = minhits;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "iou-threshold" => {
                let settings = self.settings.lock().unwrap();
                settings.iou_threshold.to_value()
            }
            "maxage" => {
                let settings = self.settings.lock().unwrap();
                settings.maxage.to_value()
            }
            "minhits" => {
                let settings = self.settings.lock().unwrap();
                settings.minhits.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for SortTracker {}

impl ElementImpl for SortTracker {
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

impl BaseTransformImpl for SortTracker {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::NeverInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;

    fn set_caps(
        &self,
        element: &Self::Type,
        incaps: &gst::Caps,
        outcaps: &gst::Caps,
    ) -> Result<(), gst::LoggableError> {
        let structure = outcaps.structure(0).unwrap();
        let width = structure.get::<i32>("width").unwrap();
        let height = structure.get::<i32>("height").unwrap();

        let settings = self.settings.lock().unwrap();
        *self.sort.lock().unwrap() = Some(Sort::new(
            height as usize,
            width as usize,
            settings.maxage as u64,
            settings.minhits as u64,
            settings.iou_threshold,
        ));

        gst_debug!(
            CAT,
            obj: element,
            "Configured for caps {} to {}",
            incaps,
            outcaps
        );
        Ok(())
    }

    fn transform(
        &self,
        _element: &Self::Type,
        inbuf: &gst::Buffer,
        outbuf: &mut gst::BufferRef,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let bboxes = {
            let map = inbuf.map_readable().unwrap();
            Bbox::deserialize_vec(&map[..]).unwrap()
        };

        let mut sort_guard = self.sort.lock().unwrap();
        let sort = sort_guard.as_mut().unwrap();

        let pts = inbuf.pts().unwrap();
        gst_trace!(CAT, "Buffer at time {}", pts.nseconds());
        let dead_tracks = sort.update(bboxes, pts.nseconds()).unwrap();
        let dead_bboxes = dead_tracks
            .into_iter()
            .flat_map(|trk| trk.history)
            .collect();

        let encoded = Bbox::serialize_vec(&dead_bboxes);
        gst_trace!(CAT, "Encoded dead tracks into {} bytes", encoded.len());

        outbuf.set_size(encoded.len());
        let mut map = outbuf.map_writable().unwrap();
        map.copy_from_slice(&encoded[..]);

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, element: &Self::Type, event: gst::Event) -> bool {
        if let gst::EventView::Eos(_) = event.view() {
            let mut sort = self.sort.lock().unwrap();
            let final_trackers = sort.as_mut().unwrap().finalize();
            let final_bboxes = final_trackers
                .into_iter()
                .flat_map(|trk| trk.history)
                .collect();
            let encoded = Bbox::serialize_vec(&final_bboxes);

            let mut buffer = gst::Buffer::with_size(encoded.len()).unwrap();
            {
                let mut map = buffer.get_mut().unwrap().map_writable().unwrap();
                map.copy_from_slice(&encoded[..]);
            }
            let src_pad = &element.src_pads()[0];
            src_pad.push(buffer).unwrap();
        };
        self.parent_sink_event(element, event)
    }

    fn transform_caps(
        &self,
        element: &Self::Type,
        direction: gst::PadDirection,
        caps: &gst::Caps,
        _filter: Option<&gst::Caps>,
    ) -> Option<gst::Caps> {
        let other_caps = caps.clone();

        gst_debug!(
            CAT,
            obj: element,
            "Transformed caps from {} to {} in direction {:?}",
            caps,
            other_caps,
            direction
        );

        // if let Some(filter) = filter {
        //     Some(filter.intersect_with_mode(&other_caps, gst::CapsIntersectMode::First))
        // } else {
        //     Some(other_caps)
        // }
        Some(other_caps)
    }

    fn transform_size(
        &self,
        _element: &Self::Type,
        _direction: gst::PadDirection,
        _caps: &gst::Caps,
        _size: usize,
        _othercaps: &gst::Caps,
    ) -> Option<usize> {
        // FIXME: constant 2MB for now
        Some(1 << (10 + 10 + 1))
    }
}
