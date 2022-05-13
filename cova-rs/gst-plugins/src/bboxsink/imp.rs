use crate::utils::file_location::FileLocation;
use bbox::Bbox;
use gst::glib;
use gst::prelude::*;
use gst::{gst_error, gst_info};
use gst_base::subclass::prelude::*;
use once_cell::sync::Lazy;
use std::fs::File;
use std::io::BufWriter;
use std::sync::Mutex;

const DEFAULT_LOCATION: Option<FileLocation> = None;

#[derive(Debug)]
struct Settings {
    location: Option<FileLocation>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            location: DEFAULT_LOCATION,
        }
    }
}

enum State {
    Stopped,
    Started {
        writer: csv::Writer<BufWriter<File>>,
    },
}

impl Default for State {
    fn default() -> State {
        State::Stopped
    }
}

#[derive(Default)]
pub struct BboxSink {
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "bboxsink",
        gst::DebugColorFlags::empty(),
        Some("TFRecord Sink"),
    )
});

impl BboxSink {
    fn set_location(
        &self,
        element: &super::BboxSink,
        location: Option<FileLocation>,
    ) -> Result<(), glib::Error> {
        let state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            return Err(glib::Error::new(
                gst::URIError::BadState,
                "Changing the `location` property on a started `bboxsink` is not supported",
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
}

#[glib::object_subclass]
impl ObjectSubclass for BboxSink {
    const NAME: &'static str = "BboxSink";
    type Type = super::BboxSink;
    type ParentType = gst_base::BaseSink;
}

impl ObjectImpl for BboxSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![glib::ParamSpecString::new(
                "location",
                "File Location",
                "Location of the TFRecord file to write",
                None,
                glib::ParamFlags::READWRITE,
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
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for BboxSink {}

impl ElementImpl for BboxSink {
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
            let caps = gst::Caps::builder("bbox")
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

impl BaseSinkImpl for BboxSink {
    fn start(&self, element: &Self::Type) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            unreachable!("BboxSink already started");
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
            2 * 1024 * 1024,
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
        let writer = csv::Writer::from_writer(writer);
        // gst_debug!(CAT, obj: element, "Opened writer {:?}", writer);

        *state = State::Started { writer };
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
                    ["BboxSink not started"]
                ));
            }
        }

        *state = State::Stopped;
        gst_info!(CAT, obj: element, "Stopped");

        Ok(())
    }

    fn render(
        &self,
        _element: &Self::Type,
        buffer: &gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();
        let writer = match *state {
            State::Started { ref mut writer } => writer,
            State::Stopped => unreachable!(),
        };

        let map = buffer.map_readable().unwrap();
        let bboxes: Vec<Bbox> = Bbox::deserialize_vec(&map[..]).unwrap();
        for bbox in bboxes {
            writer.serialize(bbox).map_err(|_| gst::FlowError::Error)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }
}
