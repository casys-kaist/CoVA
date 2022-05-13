use bbox::{Bbox, Frame};
use sort::Sort;
use std::io::{BufWriter, Write};
use std::net::TcpStream;
use tokio_util::codec::{Encoder, LengthDelimitedCodec};

/// Wrapper around SORT tracker
pub struct Tracker {
    sort: Sort,
    socket: Option<BufWriter<TcpStream>>,
    codec: LengthDelimitedCodec,
}

impl Tracker {
    pub fn new(
        port: u32,
        height: usize,
        width: usize,
        max_age: u64,
        min_hits: u64,
        iou_threshold: f64,
    ) -> Self {
        let socket = (port != 0).then(|| {
            BufWriter::new(
                TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Socket Creation Failed"),
            )
        });

        Tracker {
            sort: Sort::new(
                height,
                width,
                max_age,
                min_hits,
                iou_threshold as sort::PrecisionType,
            ),
            socket,
            codec: LengthDelimitedCodec::new(),
        }
    }
    pub fn update(&mut self, bboxes: Vec<Bbox>, pts: u64) -> Option<u64> {
        // Update
        let dead_tracks = self.sort.update(bboxes, pts).unwrap();

        // Calculate the optimal timestamp that needs to be decoded
        let ret = if dead_tracks.len() != 0 {
            Some(
                dead_tracks
                    .iter()
                    .filter(|trk| !trk.is_seen())
                    .fold(0, |max, trk| std::cmp::max(max, trk.start)),
            )
        } else {
            None
        };

        let mut buf = bytes::BytesMut::new();

        let oldest = self.get_oldest_timestamp();
        if self.socket.is_some() {
            dead_tracks.iter().for_each(|trk| {
                let tracks = trk.history.iter().map(|trk| trk.clone()).collect();
                let bytes = Frame {
                    oldest,
                    bboxes: tracks,
                }
                .ser();
                self.codec.encode(bytes.into(), &mut buf).unwrap();
                self.socket
                    .as_mut()
                    .unwrap()
                    .write(&buf)
                    .expect("Failed to write bbox through socket");
            });
        }
        ret
    }

    fn get_oldest_timestamp(&self) -> u64 {
        self.sort
            .trackers
            .iter()
            .fold(0, |min, trk| std::cmp::min(min, trk.start))
    }

    pub fn seen(&mut self, pts: u64) {
        self.sort.mark_seen(pts);
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        if let Some(ref mut writer) = self.socket {
            return writer.flush();
        }
        Ok(())
    }
}
