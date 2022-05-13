use gst::buffer::Buffer;
use gst::bufferlist::BufferList;
use gst::glib;
use gst::prelude::*;
use gst::{gst_info, gst_log, gst_trace, BufferFlags, ClockTime};
use gst_base::subclass::prelude::*;
use std::collections::LinkedList;

use std::i32;
use std::sync::Mutex;

use super::tracker::Tracker;
use bbox::Bbox;

use once_cell::sync::Lazy;
use std::ops::DerefMut;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new("cova", gst::DebugColorFlags::empty(), Some("CoVA plugin"))
});

const DEFAULT_SORT_IOU: f32 = 0.1;
const DEFAULT_SORT_MAXAGE: u32 = 30;
const DEFAULT_SORT_MINHITS: u32 = 30;
const DEFAULT_PORT: u32 = 0;
const DEFAULT_INFER_I: bool = false;
const DEFAULT_DEBUG: bool = false;
const DEFAULT_ALPHA: u32 = 0;
const DEFAULT_BETA: u32 = 0;

#[derive(Debug, Clone)]
struct Settings {
    sort_iou: f32,
    sort_maxage: u32,
    sort_minhits: u32,
    port: u32,
    debug: bool,
    infer_i: bool,
    alpha: u32,
    beta: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            sort_iou: DEFAULT_SORT_IOU,
            sort_maxage: DEFAULT_SORT_MAXAGE,
            sort_minhits: DEFAULT_SORT_MINHITS,
            port: DEFAULT_PORT,
            debug: DEFAULT_DEBUG,
            infer_i: DEFAULT_INFER_I,
            alpha: DEFAULT_ALPHA,
            beta: DEFAULT_BETA,
        }
    }
}

#[derive(Default)]
struct State {
    /// (min, max, in, out, finalized)
    bufs: LinkedList<(
        ClockTime,
        ClockTime,
        LinkedList<Buffer>,
        LinkedList<Buffer>,
        bool,
    )>,
    tracker: Option<Tracker>,
}

#[derive(Default)]
struct Counts {
    decoded_dependency: usize,
    decoded_inference: usize,
    dropped: usize,
}

pub struct Cova {
    src_pad: gst::Pad,
    sink_mask_pad: gst::Pad,
    sink_enc_pad: gst::Pad,
    eos: Mutex<[bool; 2]>,
    settings: Mutex<Settings>,
    state: Mutex<State>,
    counts: Mutex<Counts>,
}

impl Cova {
    /// Called when encoded buffer arrives
    fn sink_mask_chain(
        &self,
        _pad: &gst::Pad,
        element: &super::Cova,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.deref_mut();
        let settings = self.settings.lock().unwrap();
        let tracker = state.tracker.get_or_insert_with(|| {
            Tracker::new(
                settings.port,
                45,
                80,
                (*settings).sort_maxage as u64,
                (*settings).sort_minhits as u64,
                (*settings).sort_iou as f64,
            )
        });

        let pts = buffer.pts().unwrap();
        let map = buffer.map_readable().unwrap();
        let bboxes: Vec<Bbox> = Bbox::deserialize_vec(&map[..]).unwrap();
        gst_trace!(
            CAT,
            obj: element,
            "{} bounding boxes recived on PTS: {}",
            bboxes.len(),
            pts
        );

        // Update tracker
        let min_required = tracker.update(bboxes, pts.nseconds());

        // Calculate droppable PTS
        const SAFETY_BUFFER: u64 = 10;
        let clocktime_30fps = ClockTime::SECOND / 30;
        let maxage_pts: ClockTime = clocktime_30fps * (settings.sort_maxage as u64 + SAFETY_BUFFER);
        let max_track_pts = if pts >= maxage_pts {
            pts - maxage_pts
        } else {
            ClockTime::ZERO
        };

        // Check if there was unseen object
        if let Some(min_track_pts) = min_required {
            // There was an objectd missed
            let min_track_pts = ClockTime::from_nseconds(min_track_pts);
            gst_log!(
                CAT,
                obj: element,
                "Decoding required in {} ~ {}",
                min_track_pts,
                max_track_pts
            );
            let mut track_inferenced: usize = 0;
            let mut decoded_dependency = 0;
            let mut decoded_inference = 0;
            for (min, max, gop, out, _) in state
                .bufs
                .iter_mut()
                .rev() // Interate in reverse order for optimal decoding
                .filter(|(min, max, _, _, _)| &min_track_pts <= max && min <= &max_track_pts)
            {
                // Check if the required frame is already in output bufferlist
                if out.iter_mut().any(|buf| {
                    if min_track_pts < buf.pts().unwrap() {
                        // buf.get_mut().unwrap().unset_flags(gst::BufferFlags::DROPPABLE);
                        track_inferenced += 1;
                        // let mut counts = self.counts.lock().unwrap();
                        // counts.decoded_inference += 1;
                        // counts.decoded_dependency -= 1;
                        true
                    } else {
                        false
                    }
                }) {
                    continue;
                }

                while let Some(mut buf) = gop.pop_front() {
                    let cur_pts = buf.pts().unwrap();
                    if track_inferenced > 0 {
                        // If the track is inferenced in any previous GoP
                        break;
                    }

                    if min_track_pts <= cur_pts {
                        tracker.seen(cur_pts.nseconds());
                        decoded_inference += 1;
                        out.push_back(buf);
                        track_inferenced += 1;
                        break;
                    } else {
                        buf.make_mut().set_flags(gst::BufferFlags::DROPPABLE);
                        decoded_dependency += 1;
                        out.push_back(buf);
                    }
                }
                gst_log!(
                    CAT,
                    obj: element,
                    "In GoP [{} ~ {}], Decoded {} frames, Inferenced {} frame",
                    min,
                    max,
                    decoded_dependency + decoded_inference,
                    decoded_inference
                );
            }

            if track_inferenced < settings.beta as usize {
                state
                    .bufs
                    .iter_mut()
                    .rev()
                    .filter(|(min, max, _, _, _)| &min_track_pts <= max && min <= &max_track_pts)
                    .filter(|(_, _, _, out, _)| !out.is_empty())
                    .for_each(|(_, _, ref mut gop, ref mut out, _)| {
                        // Perform extra decoding
                        let extra_decode = usize::min(gop.len(), settings.alpha as usize);
                        let extra_infer =
                            usize::min(extra_decode, settings.beta as usize - track_inferenced);

                        if extra_decode == 0 || extra_infer == 0 {
                            return;
                        }

                        let step_extra_infer = extra_decode / extra_infer;
                        let remainder = extra_decode % extra_infer;

                        for _ in 0..remainder {
                            let mut buf = gop.pop_front().unwrap();
                            buf.get_mut()
                                .unwrap()
                                .set_flags(gst::BufferFlags::DROPPABLE);
                            decoded_dependency += 1;
                            out.push_back(buf);
                        }

                        for _ in 0..extra_infer {
                            let num_dependant = step_extra_infer.saturating_sub(1);
                            for _ in 0..num_dependant {
                                let mut buf = gop.pop_front().unwrap();
                                buf.get_mut()
                                    .unwrap()
                                    .set_flags(gst::BufferFlags::DROPPABLE);
                                decoded_dependency += 1;
                                out.push_back(buf);
                            }
                            let buf = gop.pop_front().unwrap();
                            tracker.seen(buf.pts().unwrap().nseconds());
                            decoded_inference += 1;
                            out.push_back(buf);
                            track_inferenced += 1;
                        }
                    });
            }
            assert!(track_inferenced > 0);
            {
                let mut count = self.counts.lock().unwrap();
                count.decoded_inference += decoded_inference;
                count.decoded_dependency += decoded_dependency;
            }
        }

        let mut dropped = 0;
        let mut decoded_inference = 0;

        let gop_pts = ClockTime::SECOND / 30 * 250;
        let mut droppable_pts = if pts >= gop_pts {
            pts - gop_pts
        } else {
            ClockTime::ZERO
        };

        let droppable_gops = state
            .bufs
            .drain_filter(|(_, max, _, _, finalized)| *finalized && max <= &mut droppable_pts);
        for (min, max, mut gop, mut out, _) in droppable_gops {
            gst_log!(
                CAT,
                obj: element,
                "Given that current PTS is {}, GoP [{} ~ {}] is considered droppable",
                pts,
                min,
                max
            );

            if settings.infer_i {
                if let Some(buf) = gop.pop_front() {
                    if !buf.flags().contains(BufferFlags::DELTA_UNIT) {
                        decoded_inference += 1;
                        out.push_back(buf);
                    } else {
                        dropped += 1;
                    }
                }
            }

            if !out.is_empty() {
                let mut bufferlist = BufferList::new();
                {
                    let bufferlist_ref = bufferlist.get_mut().unwrap();
                    while let Some(buf) = out.pop_front() {
                        bufferlist_ref.add(buf);
                    }
                }
                self.src_pad.push_list(bufferlist)?;
            };
            dropped += gop.len();
        }

        if dropped != 0 || decoded_inference != 0 {
            gst_info!(
                CAT,
                obj: element,
                "Found droppable GoP \
                inference decoded: {}, \
                dropped: {} frames",
                decoded_inference,
                dropped
            );
            let mut count = self.counts.lock().unwrap();
            count.decoded_inference += decoded_inference;
            count.dropped += dropped;
        }
        Ok(gst::FlowSuccess::Ok)
    }

    /// Called when encoded buffer arrives
    fn sink_enc_chain(
        &self,
        _pad: &gst::Pad,
        element: &super::Cova,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();
        if !buffer.flags().contains(BufferFlags::DELTA_UNIT) {
            // Mark previous GoP is finalized
            if let Some(mut back) = state.bufs.back_mut() {
                back.4 = true;
            }

            let mut new_gop_list = LinkedList::new();
            let pts = buffer.pts().unwrap();
            let mut buffer = buffer.copy();
            buffer.get_mut().unwrap().set_flags(BufferFlags::DISCONT);
            new_gop_list.push_back(buffer);
            gst_trace!(
                CAT,
                obj: element,
                "IDR frame encoundtered at [PTS: {}]",
                pts
            );
            state
                .bufs
                .push_back((pts, pts, new_gop_list, LinkedList::new(), false));
        } else {
            let min_pts = state.bufs.back().unwrap().0;
            let max_pts = state.bufs.back().unwrap().1;
            let cur_pts = buffer.pts().unwrap();
            let back = state.bufs.back_mut().unwrap();
            if cur_pts < min_pts {
                back.0 = cur_pts;
            } else if cur_pts > max_pts {
                back.1 = cur_pts;
            }
            back.2.push_back(buffer);
        }
        Ok(gst::FlowSuccess::Ok)
    }
    fn sink_mask_event(&self, _pad: &gst::Pad, element: &super::Cova, event: gst::Event) -> bool {
        if let gst::EventView::Eos(eos_event) = event.view() {
            if eos_event.is_downstream() {
                let mut eos = self.eos.lock().unwrap();
                eos[1] = true;
                gst_info!(CAT, obj: element, "EOS recieved from sink_mask");

                if eos[0] && eos[1] {
                    gst_info!(CAT, obj: element, "EOS recieved from both side");
                    let mut dropped = 0;
                    self.state
                        .lock()
                        .unwrap()
                        .bufs
                        .drain_filter(|_| true)
                        .for_each(|(_, _, gop, mut out, _)| {
                            dropped += gop.len();
                            let mut bufferlist = BufferList::new();
                            {
                                let bufferlist_ref = bufferlist.get_mut().unwrap();
                                while let Some(buf) = out.pop_front() {
                                    bufferlist_ref.add(buf);
                                }
                            }
                            self.src_pad.push_list(bufferlist).unwrap();
                        });
                    self.counts.lock().unwrap().dropped += dropped;
                    let mut tracker = self.state.lock().unwrap().tracker.take();
                    if let Some(ref mut tracker) = tracker {
                        tracker.flush().unwrap();
                    }

                    return self.src_pad.push_event(event);
                }
            }
        };
        true
    }
    fn sink_enc_event(&self, _pad: &gst::Pad, element: &super::Cova, event: gst::Event) -> bool {
        if let gst::EventView::Eos(eos_event) = event.view() {
            let mut eos = self.eos.lock().unwrap();
            if eos_event.is_downstream() {
                eos[0] = true;
                gst_info!(CAT, obj: element, "EOS recieved from sink_enc");

                if eos[0] && eos[1] {
                    gst_info!(CAT, obj: element, "EOS recieved from both side");
                    let mut dropped = 0;
                    self.state
                        .lock()
                        .unwrap()
                        .bufs
                        .drain_filter(|_| true)
                        .for_each(|(_, _, gop, mut out, _)| {
                            dropped += gop.len();
                            let mut bufferlist = BufferList::new();
                            {
                                let bufferlist_ref = bufferlist.get_mut().unwrap();
                                while let Some(buf) = out.pop_front() {
                                    bufferlist_ref.add(buf);
                                }
                            }
                            self.src_pad.push_list(bufferlist).unwrap();
                        });
                    self.counts.lock().unwrap().dropped += dropped;
                } else {
                    return true;
                }
            }
        };
        self.src_pad.push_event(event)
    }
    fn sink_query(
        &self,
        _pad: &gst::Pad,
        _element: &super::Cova,
        query: &mut gst::QueryRef,
    ) -> bool {
        self.src_pad.peer_query(query)
    }
    fn src_event(&self, _pad: &gst::Pad, _element: &super::Cova, event: gst::Event) -> bool {
        if let gst::EventView::Caps(_) = event.view() {
            self.sink_enc_pad.push_event(event)
        } else {
            self.sink_mask_pad.push_event(event.clone()) && self.sink_enc_pad.push_event(event)
        }
    }
    fn src_query(
        &self,
        _pad: &gst::Pad,
        _element: &super::Cova,
        query: &mut gst::QueryRef,
    ) -> bool {
        self.sink_enc_pad.peer_query(query)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Cova {
    const NAME: &'static str = "Cova";
    type Type = super::Cova;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink_mask").unwrap();
        let sink_mask_pad = gst::Pad::builder_with_template(&templ, Some("sink_mask"))
            .chain_function(|pad, parent, buffer| {
                Cova::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |cova, element| cova.sink_mask_chain(pad, element, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Cova::catch_panic_pad_function(
                    parent,
                    || false,
                    |cova, element| cova.sink_mask_event(pad, element, event),
                )
            })
            .build();
        let templ = klass.pad_template("sink_enc").unwrap();
        let sink_enc_pad = gst::Pad::builder_with_template(&templ, Some("sink_enc"))
            .chain_function(|pad, parent, buffer| {
                Cova::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |cova, element| cova.sink_enc_chain(pad, element, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Cova::catch_panic_pad_function(
                    parent,
                    || false,
                    |cova, element| cova.sink_enc_event(pad, element, event),
                )
            })
            .query_function(|pad, parent, query| {
                Cova::catch_panic_pad_function(
                    parent,
                    || false,
                    |cova, element| cova.sink_query(pad, element, query),
                )
            })
            .build();

        let src_pad =
            gst::Pad::builder_with_template(&klass.pad_template("src").unwrap(), Some("src"))
                .event_function(|pad, parent, event| {
                    Cova::catch_panic_pad_function(
                        parent,
                        || false,
                        |cova, element| cova.src_event(pad, element, event),
                    )
                })
                .query_function(|pad, parent, query| {
                    Cova::catch_panic_pad_function(
                        parent,
                        || false,
                        |cova, element| cova.src_query(pad, element, query),
                    )
                })
                .build();
        Self {
            src_pad,
            sink_mask_pad,
            sink_enc_pad,
            eos: Mutex::new([false, false]),
            settings: Mutex::new(Settings::default()),
            state: Mutex::new(State::default()),
            counts: Mutex::new(Counts::default()),
        }
    }
}

impl ObjectImpl for Cova {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpecFloat::new(
                    "sort-iou",
                    "SORT IoU",
                    "IoU threshold used by SORT",
                    0.,
                    1.,
                    DEFAULT_SORT_IOU,
                    glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecUInt::new(
                    "sort-maxage",
                    "SORT Max Age",
                    "Max age parameter used by SORT",
                    0,
                    u32::MAX,
                    DEFAULT_SORT_MAXAGE,
                    glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecUInt::new(
                    "sort-minhits",
                    "Track Min Hits",
                    "Min hits parameter used by SORT",
                    0,
                    u32::MAX,
                    DEFAULT_SORT_MINHITS,
                    glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecUInt::new(
                    "port",
                    "Port",
                    "TCP port number for attatching to Aggregator (0: disabled)",
                    0,
                    u32::MAX,
                    DEFAULT_PORT,
                    glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecBoolean::new(
                    "infer-i",
                    "[DEPRECATED] Infer I frame",
                    "[DEPRECATED] Run inference on I frames",
                    DEFAULT_INFER_I,
                    glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecBoolean::new(
                    "debug",
                    "Debug",
                    "Run in debug mode",
                    DEFAULT_DEBUG,
                    glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecUInt::new(
                    "alpha",
                    "Alpha Parameter",
                    "Parameter setting how many extra frames is sent for decoding",
                    0,
                    u32::MAX,
                    DEFAULT_SORT_MINHITS,
                    glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecUInt::new(
                    "beta",
                    "Beta Parameter",
                    "Parameter setting how many extra frames is sent for inferencing",
                    0,
                    u32::MAX,
                    DEFAULT_SORT_MINHITS,
                    glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecUInt64::new(
                    "dropped",
                    "Dropped frame counts",
                    "Dropped frame counts",
                    0,
                    u64::MAX,
                    0,
                    glib::ParamFlags::READABLE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecUInt64::new(
                    "decoded-dependency",
                    "Decoded for dependency counts",
                    "Number of decoded frames for dependency",
                    0,
                    u64::MAX,
                    0,
                    glib::ParamFlags::READABLE | gst::PARAM_FLAG_MUTABLE_PLAYING,
                ),
                glib::ParamSpecUInt64::new(
                    "decoded-inference",
                    "Decoded for inference counts",
                    "Number of decoded frames for inference",
                    0,
                    u64::MAX,
                    0,
                    glib::ParamFlags::READABLE | gst::PARAM_FLAG_MUTABLE_PLAYING,
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
            "sort-iou" => {
                let mut settings = self.settings.lock().unwrap();
                let sort_iou = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing sort-iou from {} to {}",
                    settings.sort_iou,
                    sort_iou
                );
                settings.sort_iou = sort_iou;
            }
            "sort-maxage" => {
                let mut settings = self.settings.lock().unwrap();
                let sort_maxage = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing sort-maxage from {} to {}",
                    settings.sort_maxage,
                    sort_maxage
                );
                settings.sort_maxage = sort_maxage;
            }
            "sort-minhits" => {
                let mut settings = self.settings.lock().unwrap();
                let sort_minhits = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing sort-minhits from {} to {}",
                    settings.sort_minhits,
                    sort_minhits
                );
                settings.sort_minhits = sort_minhits;
            }
            "port" => {
                let mut settings = self.settings.lock().unwrap();
                let port = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing port from {} to {}",
                    settings.port,
                    port
                );
                settings.port = port;
            }
            "infer-i" => {
                let mut settings = self.settings.lock().unwrap();
                let infer_i = value.get().expect("type checked upstream");
                settings.infer_i = infer_i;
            }
            "debug" => {
                let mut settings = self.settings.lock().unwrap();
                let debug = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing debug from {} to {}",
                    settings.debug,
                    debug
                );
                settings.debug = debug;
            }
            "alpha" => {
                let mut settings = self.settings.lock().unwrap();
                let alpha = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing aplha from {} to {}",
                    settings.alpha,
                    alpha
                );
                settings.alpha = alpha;
            }
            "beta" => {
                let mut settings = self.settings.lock().unwrap();
                let beta = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing beta from {} to {}",
                    settings.beta,
                    beta
                );
                settings.beta = beta;
            }
            "dropped" | "decoded-dependency" | "decoded-inference" => (),
            _ => unimplemented!(),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "sort-iou" => {
                let settings = self.settings.lock().unwrap();
                settings.sort_iou.to_value()
            }
            "sort-maxage" => {
                let settings = self.settings.lock().unwrap();
                settings.sort_maxage.to_value()
            }
            "sort-minhits" => {
                let settings = self.settings.lock().unwrap();
                settings.sort_minhits.to_value()
            }
            "port" => {
                let settings = self.settings.lock().unwrap();
                settings.port.to_value()
            }
            "infer-i" => {
                let settings = self.settings.lock().unwrap();
                settings.infer_i.to_value()
            }
            "debug" => {
                let settings = self.settings.lock().unwrap();
                settings.debug.to_value()
            }
            "alpha" => {
                let settings = self.settings.lock().unwrap();
                settings.alpha.to_value()
            }
            "beta" => {
                let settings = self.settings.lock().unwrap();
                settings.beta.to_value()
            }
            "dropped" => {
                let counts = self.counts.lock().unwrap();
                (counts.dropped as u64).to_value()
            }
            "decoded-dependency" => {
                let counts = self.counts.lock().unwrap();
                (counts.decoded_dependency as u64).to_value()
            }
            "decoded-inference" => {
                let counts = self.counts.lock().unwrap();
                (counts.decoded_inference as u64).to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);

        obj.add_pad(&self.src_pad).unwrap();
        obj.add_pad(&self.sink_mask_pad).unwrap();
        obj.add_pad(&self.sink_enc_pad).unwrap();
    }

    // FIXME: the dispose function is not called for some reason.
    // fn dispose(&self, _obj: &Self::Type) {
    //     let counts = self.counts.lock().unwrap();
    //     println!(
    //         "decoded_inference:{},decoded_dependency:{},dropped:{}",
    //         counts.decoded_inference, counts.decoded_dependency, counts.dropped
    //     );
    //     let mut tracker = self.state.lock().unwrap().tracker.take();
    //     if let Some(ref mut tracker) = tracker {
    //         tracker.flush().unwrap();
    //     }
    // }
}

impl GstObjectImpl for Cova {}

impl ElementImpl for Cova {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "CoVA Filter",
                "Filter/Video",
                "Filter optimal frames to decode using SORT on extracted masks",
                "Anonymous CoVA <anonymouscova@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::new_simple(
                "bbox",
                &[
                    ("width", &gst::IntRange::<i32>::new(0, i32::MAX)),
                    ("height", &gst::IntRange::<i32>::new(0, i32::MAX)),
                ],
            );

            let sink_mask_pad_template = gst::PadTemplate::new(
                "sink_mask",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::new_any();

            let sink_enc_pad_template = gst::PadTemplate::new(
                "sink_enc",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![
                sink_mask_pad_template,
                sink_enc_pad_template,
                src_pad_template,
            ]
        });
        PAD_TEMPLATES.as_ref()
    }
}
