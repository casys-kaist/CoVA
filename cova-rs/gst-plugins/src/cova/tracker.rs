use bbox::{Bbox, Frame};
use sort::Sort;
use std::io::{BufWriter, Write};
use std::net::{Shutdown, TcpStream};
use tokio_util::codec::{Encoder, LengthDelimitedCodec};

/// Wrapper around SORT tracker
pub struct Tracker {
    sort: Sort,
    socket: Option<BufWriter<TcpStream>>,
    codec: LengthDelimitedCodec,
    range_start: Option<u64>,
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
            range_start: None,
        }
    }
    pub fn update(&mut self, bboxes: Vec<Bbox>, pts: u64) -> Option<u64> {
        // Define the starting PTS of current tracker
        let range_start = *self.range_start.get_or_insert(pts);

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
                    range_start,
                    oldest,
                    bboxes: tracks,
                }
                .ser();
                self.codec.encode(bytes.into(), &mut buf).unwrap();
                self.socket
                    .as_mut()
                    .unwrap()
                    .write(&buf)
                    .expect("writing bbox failed");
            });
        }
        ret
    }

    fn get_oldest_timestamp(&self) -> u64 {
        self.sort
            .trackers
            .iter()
            .fold(u64::MAX, |min, trk| std::cmp::min(min, trk.start))
    }

    pub fn seen(&mut self, pts: u64) {
        self.sort.mark_seen(pts);
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        let range_start = self.range_start.unwrap();
        let mut buf = bytes::BytesMut::new();

        let oldest = self.get_oldest_timestamp();
        if self.socket.is_some() {
            self.sort.finalize().iter().for_each(|trk| {
                let tracks = trk.history.iter().map(|trk| trk.clone()).collect();
                let bytes = Frame {
                    range_start,
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

        if let Some(mut writer) = self.socket.take() {
            writer.flush()?;
            writer.into_inner().unwrap().shutdown(Shutdown::Both)?;
        }
        Ok(())
    }
}
