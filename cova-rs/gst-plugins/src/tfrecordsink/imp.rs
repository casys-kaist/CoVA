use crate::utils::file_location::FileLocation;
use gst::glib;
use gst::prelude::*;
use gst::{gst_debug, gst_error, gst_info, gst_trace};
use gst_base::subclass::prelude::*;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;
use std::iter::FromIterator;
use std::sync::Mutex;
use tfrecord::{Example, ExampleWriter, Feature};

const DEFAULT_LOCATION: Option<FileLocation> = None;
const DEFAULT_GT: Option<FileLocation> = None;
const DEFAULT_GOP: u32 = 0;

#[derive(Debug)]
struct Settings {
    location: Option<FileLocation>,
    gt: Option<FileLocation>,
    gop: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            location: DEFAULT_LOCATION,
            gt: DEFAULT_GT,
            gop: DEFAULT_GOP,
        }
    }
}

enum State {
    Stopped,
    Started {
        writer: ExampleWriter<BufWriter<File>>,
        gt: File,
        batch_count: usize,
    },
}

impl Default for State {
    fn default() -> State {
        State::Stopped
    }
}

#[derive(Default)]
pub struct TFRecordSink {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    example: Mutex<Option<HashMap<String, Vec<Vec<u8>>>>>,
    video_info: Mutex<Option<gst_video::VideoInfo>>,
}

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "tfrecordsink",
        gst::DebugColorFlags::empty(),
        Some("TFRecord Sink"),
    )
});

impl TFRecordSink {
    #[inline]
    fn parse_bytes_into_example(
        &self,
        in_frame: gst_video::VideoFrameRef<&gst::BufferRef>,
    ) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        let (gt, batch_count) = match *state {
            State::Started {
                ref mut gt,
                ref mut batch_count,
                ..
            } => (gt, batch_count),
            State::Stopped => {
                unreachable!();
            }
        };

        let mut example_guard = self.example.lock().unwrap();
        let example = example_guard.get_or_insert_with(|| TFRecordSink::new_example(0));

        let (width, height) = {
            let video_info = self.video_info.lock().unwrap();
            (
                video_info.as_ref().unwrap().width() as usize,
                video_info.as_ref().unwrap().height() as usize,
            )
        };

        let map = in_frame.plane_data(0).unwrap();
        let stride = in_frame.plane_stride()[0] as usize;
        let line_bytes = (in_frame.width() * 4) as usize;

        let mut vec_mb_type = Vec::with_capacity(width * height);
        let mut vec_mv_x = Vec::with_capacity(width * height);
        let mut vec_mv_y = Vec::with_capacity(width * height);
        let mut vec_gt = Vec::with_capacity(width * height);

        for chunk in map
            .chunks_exact(stride)
            .flat_map(|l| l[..line_bytes].chunks_exact(4))
        {
            vec_mb_type.push(chunk[0]);
            vec_mv_x.push(chunk[1]);
            vec_mv_y.push(chunk[2]);
        }
        unsafe {
            vec_gt.set_len(vec_gt.capacity());
        }

        gt.read_exact(&mut vec_gt).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::Read,
                [
                    "Could not read file from ground truth file: {}",
                    err.to_string(),
                ]
            )
        })?;

        example.get_mut("mb_type").unwrap().push(vec_mb_type);
        example.get_mut("mv_x").unwrap().push(vec_mv_x);
        example.get_mut("mv_y").unwrap().push(vec_mv_y);
        example.get_mut("gt").unwrap().push(vec_gt);

        *batch_count += 1;
        Ok(())
    }

    /// Write down Example into TF Record
    /// If current GoP did not meet the preset value, zero fill the rest
    fn write(&self) -> Result<(), tfrecord::Error> {
        let mut state = self.state.lock().unwrap();
        let (writer, batch_count) = match *state {
            State::Started {
                ref mut writer,
                ref mut batch_count,
                ..
            } => (writer, batch_count),
            State::Stopped => {
                unreachable!();
            }
        };

        let mut example_guard = self.example.lock().unwrap();
        let mut example = example_guard
            .take()
            .expect("example is not initialized before writting");

        let gop = self.settings.lock().unwrap().gop as usize;
        if gop != 0 {
            let missing = gop - *batch_count;
            if missing != 0 {
                let (width, height) = {
                    let video_info = self.video_info.lock().unwrap();
                    (
                        video_info.as_ref().unwrap().width() as usize,
                        video_info.as_ref().unwrap().height() as usize,
                    )
                };
                TFRecordSink::zero_fill_example(&mut example, missing, width * height);
            }
        }

        writer.send(Example::from_iter(
            example
                .into_iter()
                .map(|(name, feature)| (name, Feature::from_bytes_list(feature))),
        ))?;
        *batch_count = 0;

        Ok(())
    }

    fn zero_fill_example(example: &mut HashMap<String, Vec<Vec<u8>>>, count: usize, size: usize) {
        {
            let v = example.get_mut("mb_type").unwrap();
            (0..count).for_each(|_| v.push(vec![0; size]));
        }
        {
            let v = example.get_mut("mv_x").unwrap();
            (0..count).for_each(|_| v.push(vec![0; size]));
        }
        {
            let v = example.get_mut("mv_y").unwrap();
            (0..count).for_each(|_| v.push(vec![0; size]));
        }
        {
            let v = example.get_mut("gt").unwrap();
            (0..count).for_each(|_| v.push(vec![0; size]));
        }
    }

    fn new_example(size: usize) -> HashMap<String, Vec<Vec<u8>>> {
        HashMap::from_iter([
            ("mb_type".into(), Vec::with_capacity(size)),
            ("mv_x".into(), Vec::with_capacity(size)),
            ("mv_y".into(), Vec::with_capacity(size)),
            ("gt".into(), Vec::with_capacity(size)),
        ])
    }

    fn set_location(
        &self,
        element: &super::TFRecordSink,
        location: Option<FileLocation>,
    ) -> Result<(), glib::Error> {
        let state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            return Err(glib::Error::new(
                gst::URIError::BadState,
                "Changing the `location` property on a started `tfrecordsink` is not supported",
            ));
        }

        let mut settings = self.settings.lock().unwrap();
        settings.location = match location {
            Some(location) => {
                match settings.location {
                    Some(ref location_cur) => {
                        gst_info!(
                            CAT,
                            obj: element,
                            "Changing `location` from {:?} to {}",
                            location_cur,
                            location,
                        );
                    }
                    None => {
                        gst_info!(CAT, obj: element, "Setting `location` to {}", location,);
                    }
                }
                Some(location)
            }
            None => {
                gst_info!(CAT, obj: element, "Resetting `location` to None",);
                None
            }
        };

        Ok(())
    }

    fn set_gt(
        &self,
        element: &super::TFRecordSink,
        gt: Option<FileLocation>,
    ) -> Result<(), glib::Error> {
        let state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            return Err(glib::Error::new(
                gst::URIError::BadState,
                "Changing the `gt` property on a started `tfrecordsink` is not supported",
            ));
        }

        let mut settings = self.settings.lock().unwrap();
        settings.gt = match gt {
            Some(gt) => {
                match settings.gt {
                    Some(ref gt_cur) => {
                        gst_info!(
                            CAT,
                            obj: element,
                            "Changing `gt` from {:?} to {}",
                            gt_cur,
                            gt,
                        );
                    }
                    None => {
                        gst_info!(CAT, obj: element, "Setting `gt` to {}", gt,);
                    }
                }
                Some(gt)
            }
            None => {
                gst_info!(CAT, obj: element, "Resetting `gt` to None",);
                None
            }
        };

        Ok(())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TFRecordSink {
    const NAME: &'static str = "TFRecordSink";
    type Type = super::TFRecordSink;
    type ParentType = gst_base::BaseSink;
}

impl ObjectImpl for TFRecordSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpecString::new(
                    "location",
                    "File Location",
                    "Location of the TFRecord file to write",
                    None,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpecString::new(
                    "gt",
                    "Ground Truth Location",
                    "Location of the ground truth label",
                    None,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpecUInt::new(
                    "gop",
                    "Group of Pictures",
                    "Stacks records into the value at GoP boundary, zero filled if count does not match (0 = disabled)",
                    0,
                    u32::MAX,
                    DEFAULT_GOP,
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
            "location" => {
                let res = match value.get::<Option<String>>() {
                    Ok(Some(location)) => FileLocation::try_from_path_str(location)
                        .and_then(|file_location| self.set_location(obj, Some(file_location))),
                    Ok(None) => self.set_location(obj, None),
                    Err(_) => unreachable!("type checked upstream"),
                };

                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `location`: {}", err);
                }
            }
            "gt" => {
                let res = match value.get::<Option<String>>() {
                    Ok(Some(gt)) => FileLocation::try_from_path_str(gt)
                        .and_then(|file_location| self.set_gt(obj, Some(file_location))),
                    Ok(None) => self.set_gt(obj, None),
                    Err(_) => unreachable!("type checked upstream"),
                };

                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `gt`: {}", err);
                }
            }
            "gop" => {
                let mut settings = self.settings.lock().unwrap();
                let gop = value.get().expect("type checked upstream");
                gst_info!(
                    CAT,
                    obj: obj,
                    "Changing gop from {} to {}",
                    settings.gop,
                    gop
                );
                settings.gop = gop;
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "location" => {
                let settings = self.settings.lock().unwrap();
                let location = settings
                    .location
                    .as_ref()
                    .map(|location| location.to_string());
                location.to_value()
            }
            "gt" => {
                let settings = self.settings.lock().unwrap();
                let gt = settings.gt.as_ref().map(|gt| gt.to_string());
                gt.to_value()
            }
            "gop" => self.settings.lock().unwrap().gop.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for TFRecordSink {}

impl ElementImpl for TFRecordSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "TFRecord Sink",
                "Sink/Video",
                "Pack metadatas extracted from avdec into TFRecord",
                "Anonymous CoVA <anonymouscova@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = gst::Caps::builder("video/x-raw")
                .field("format", gst_video::VideoFormat::Rgba.to_str())
                .field("width", gst::IntRange::new(0, i32::MAX))
                .field("height", gst::IntRange::new(0, i32::MAX))
                .build();

            let pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![pad_template]
        });
        PAD_TEMPLATES.as_ref()
    }
}

impl BaseSinkImpl for TFRecordSink {
    fn set_caps(&self, _element: &Self::Type, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let video_info = match gst_video::VideoInfo::from_caps(caps) {
            Err(_) => return Err(gst::loggable_error!(CAT, "Failed to parse input caps")),
            Ok(info) => info,
        };

        *self.video_info.lock().unwrap() = Some(video_info);
        Ok(())
    }

    fn start(&self, element: &Self::Type) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            unreachable!("TFRecordSink already started");
        }

        let settings = self.settings.lock().unwrap();
        let location = settings.location.as_ref().ok_or_else(|| {
            gst::error_msg!(
                gst::ResourceError::Settings,
                ["File location is not defined"]
            )
        })?;

        // 2MB buffer for now
        let writer = BufWriter::with_capacity(
            2_000_000,
            File::create(location).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenWrite,
                    [
                        "Could not open file {} for writing: {}",
                        location,
                        err.to_string(),
                    ]
                )
            })?,
        );
        let writer = ExampleWriter::from_writer(writer).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::OpenWrite,
                [
                    "Failed to create ExampleWriter from BufWriter: {}",
                    err.to_string(),
                ]
            )
        })?;

        gst_debug!(CAT, obj: element, "Opened writer {:?}", writer);

        let gt = settings.gt.as_ref().ok_or_else(|| {
            gst::error_msg!(
                gst::ResourceError::Settings,
                ["Ground truth file is not defined"]
            )
        })?;

        let gt = File::open(gt).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::OpenWrite,
                [
                    "Could not open ground truth file {}: {}",
                    gt,
                    err.to_string(),
                ]
            )
        })?;

        *state = State::Started {
            writer,
            gt,
            batch_count: 0,
        };
        gst_info!(CAT, obj: element, "Started");

        Ok(())
    }

    fn stop(&self, element: &Self::Type) -> Result<(), gst::ErrorMessage> {
        // Drop state
        let mut state = self.state.lock().unwrap();

        match *state {
            State::Started { ref mut writer, .. } => writer.flush().expect("Failed to flush"),
            State::Stopped => {
                return Err(gst::error_msg!(
                    gst::ResourceError::Settings,
                    ["TFRecordSink not started"]
                ));
            }
        }

        *state = State::Stopped;
        gst_info!(CAT, obj: element, "Stopped");

        Ok(())
    }

    fn render(
        &self,
        element: &Self::Type,
        buffer: &gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let gop = { self.settings.lock().unwrap().gop };

        // If GoP stacking is enabled and IDR buffer came in
        if gop != 0 && !buffer.flags().contains(gst::BufferFlags::DELTA_UNIT) {
            let batch_count = {
                if let State::Started { batch_count, .. } = *self.state.lock().unwrap() {
                    batch_count
                } else {
                    unreachable!();
                }
            };

            // Write back if previous batch exists
            if batch_count != 0 {
                gst_trace!(CAT, obj: element, "IDR encountered {:?}", buffer);
                self.write().map_err(|_| {
                    gst::element_error!(
                        element,
                        gst::CoreError::Failed,
                        ["Failed to write TFRecord"]
                    );
                    gst::FlowError::Error
                })?;
            }
        }

        let video_info = {
            let guard = self.video_info.lock().unwrap();
            guard.as_ref().unwrap().clone()
        };
        let in_frame =
            gst_video::VideoFrameRef::from_buffer_ref_readable(buffer.as_ref(), &video_info)
                .map_err(|_| {
                    gst::element_error!(
                        element,
                        gst::CoreError::Failed,
                        ["Failed to map input buffer readable"]
                    );
                    gst::FlowError::Error
                })?;

        self.parse_bytes_into_example(in_frame).map_err(|_| {
            gst::element_error!(
                element,
                gst::CoreError::Failed,
                ["Failed to parse bytes into TF example"]
            );
            gst::FlowError::Error
        })?;

        // If GoP stacking is disabled
        if gop == 0 {
            self.write().map_err(|_| {
                gst::element_error!(
                    element,
                    gst::CoreError::Failed,
                    ["Failed to write TFRecord"]
                );
                gst::FlowError::Error
            })?;
        }

        Ok(gst::FlowSuccess::Ok)
    }
}
