use crate::Recieved;
use bbox::{Bbox, Frame};
use itertools::Itertools;
use log::{debug, info, trace};
use std::collections::{HashMap, LinkedList};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Barrier;

#[derive(Debug)]
struct Stationary {
    range_start: u64,
    range_end: u64,
    start: u64,
    end: u64,
    bbox: Bbox,
    track_id: Option<u64>,
    class_id: u32,
}

impl Stationary {
    fn new(range_start: u64, range_end: u64, bbox: Bbox) -> Self {
        Stationary {
            range_start,
            range_end,
            start: bbox.timestamp.unwrap(),
            end: bbox.timestamp.unwrap(),
            class_id: bbox.class_id.unwrap(),
            bbox,
            track_id: None,
        }
    }

    fn update(&mut self, bbox: Bbox) {
        // Make the track longer
        self.end = bbox.timestamp.unwrap();
    }

    fn to_vec(&self) -> Vec<Bbox> {
        const TIMESTEP: u64 = 33_333_333;
        const TIMESTEP_3: usize = 100_000_000;

        let mut ret = vec![];

        for timestamp in (self.start..self.end).step_by(TIMESTEP_3) {
            for i in 0..2 {
                let timestamp = timestamp as u64 + i * TIMESTEP;

                let mut ret_bbox = self.bbox.clone();
                ret_bbox.timestamp = Some(timestamp);
                ret_bbox.track_id = self.track_id;
                ret.push(ret_bbox);
            }
        }
        ret
    }
}

struct Associator {
    /// Each trackers take care of separate range of video
    /// we track mapping from range_start to range_end
    tracker_range: HashMap<u64, u64>,
    track_writer: csv::Writer<File>,
    dnn_writer: csv::Writer<File>,
    assoc_writer: csv::Writer<File>,
    stationary_writer: csv::Writer<File>,
    /// List of (range_start, range_end, Vec<bbox>)
    tracks: LinkedList<(u64, u64, Vec<Bbox>)>,
    /// List of (matched, bbox)
    dnns: LinkedList<(bool, Bbox)>,
    /// Active stationary
    stationary: LinkedList<Stationary>,
    finalized_stationary: LinkedList<Stationary>,
    /// Mapping from track ID to list of class ID
    track2class: HashMap<u64, Vec<u32>>,
    moving_iou: f32,
    stationary_iou: f32,
    stationary_maxage: u64,
    max_track_id: u64,
    scale_factor: f32,
}

impl Associator {
    fn new(
        track_path: PathBuf,
        dnn_path: PathBuf,
        assoc_path: PathBuf,
        stationary_path: PathBuf,
        moving_iou: f32,
        stationary_iou: f32,
        stationary_maxage: usize,
        scale_factor: f32,
    ) -> Self {
        let track_writer = csv::Writer::from_writer(File::create(track_path).unwrap());
        let dnn_writer = csv::Writer::from_writer(File::create(dnn_path).unwrap());
        let assoc_writer = csv::Writer::from_writer(File::create(assoc_path).unwrap());
        let stationary_writer = csv::Writer::from_writer(File::create(stationary_path).unwrap());

        // Convert to nanosecond
        const NSEC_TO_SEC: u64 = 1_000_000_000;
        let stationary_maxage = (stationary_maxage as u64) * NSEC_TO_SEC;

        Associator {
            tracker_range: HashMap::new(),
            track_writer,
            dnn_writer,
            assoc_writer,
            stationary_writer,
            tracks: LinkedList::new(),
            dnns: LinkedList::new(),
            stationary: LinkedList::new(),
            finalized_stationary: LinkedList::new(),
            track2class: HashMap::new(),
            moving_iou,
            stationary_iou,
            stationary_maxage,
            max_track_id: 0,
            scale_factor,
        }
    }

    /// Write down tracks older than given timestamp
    fn finalize_trk(&mut self, timestamp: u64) -> Result<(), Box<dyn std::error::Error>> {
        for (range_start, range_end, mut trk) in
            self.tracks.drain_filter(|(range_start, range_end, trk)| {
                // timestamp should fall between the range
                *range_start <= timestamp && timestamp < *range_end
                        // and track should be older than the timestamp
                        && trk.last().unwrap().timestamp.unwrap() < timestamp
            })
        {
            trace!(
                "[ASSOC] Detection at {} falls between range [{}, {}), finalizing track ending at {}",
                timestamp,
                range_start,
                range_end,
                trk.last().unwrap().timestamp.unwrap()
            );

            let trk_id = trk.first().unwrap().track_id.unwrap();

            // Write back track with all associated class ID
            // if let Some(class_ids) = self.track2class.remove(&trk_id) {
            //     for class_id in class_ids {
            //         for trk_bbox in &mut trk {
            //             trk_bbox.class_id = Some(class_id);
            //             self.assoc_writer.serialize(trk_bbox)?;
            //         }
            //     }
            // };

            // Write back track with associated class
            // 1. Use most frequently associated class
            // 2. Use all classes associated more than twice
            // 3. Use all associated class if max frequency was one
            let class_ids = match self.track2class.remove(&trk_id) {
                Some(class_ids) => {
                    // Count frequency of class IDs
                    let mut count = HashMap::new();
                    for class_id in class_ids {
                        *count.entry(class_id).or_insert(0) += 1;
                    }
                    let mut class_ids: Vec<u32> = vec![];
                    let (class_id, frequency) = {
                        // Add most associated label
                        let (class_id, frequency) =
                            count.iter().max_by_key(|(_, freq)| *freq).unwrap();
                        (*class_id, *frequency)
                    };
                    count.remove(&class_id);

                    // 1. Add most frequent match
                    class_ids.push(class_id);

                    if frequency != 1 {
                        // 2. Use all classes associated more than twice
                        for (class_id, frequency) in count {
                            if frequency >= 2 {
                                class_ids.push(class_id);
                            }
                        }
                    } else {
                        // 3. Use all associated class if max frequency was one
                        for class_id in count.keys() {
                            class_ids.push(*class_id);
                        }
                    }
                    class_ids
                }
                None => {
                    // Handle unmatched tracks
                    vec![]
                }
            };

            for class_id in class_ids {
                for trk_bbox in &mut trk {
                    trk_bbox.class_id = Some(class_id);
                    self.assoc_writer.serialize(trk_bbox)?;
                }
            }
        }
        Ok(())
    }

    /// Move unmatched pending DNNs who are older than the oldest active track
    /// and make it stationary object candidate
    /// pending_dnns => stationary
    fn finalize_dnn(&mut self, range_start: u64, range_end: u64, timestamp: u64) {
        // TODO: running on the same timestamp again is a waste
        for (matched, bbox) in self.dnns.drain_filter(|(_, bbox)| {
            let dnn_timestamp = bbox.timestamp.unwrap();
            // Check the timestamp is from corresponding tracker
            range_start <= dnn_timestamp && dnn_timestamp < range_end
                    // Check if the detection has no chance for association
                    && dnn_timestamp < timestamp
        }) {
            trace!(
                "[ASSOC] Track from range [{}, {}) reports oldest timestamp of {}, finalizing DNN at {}",
                range_start,
                range_end,
                timestamp,
                bbox.timestamp.unwrap(),
            );

            // Detection never matched are candidate for stationary objects
            if !matched {
                match self
                    .stationary
                    .iter_mut()
                    // the stationary should be from the same tracker
                    .filter(|s| s.range_start == range_start)
                    // the class ID should match
                    .filter(|s| s.class_id == bbox.class_id.unwrap())
                    .map(|m| {
                        let iou = m.bbox.iou(&bbox);
                        (m, iou)
                    })
                    .filter(|(_, iou)| iou >= &self.stationary_iou)
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                {
                    Some((matched, _)) => matched.update(bbox),
                    None => {
                        self.stationary
                            .push_back(Stationary::new(range_start, range_end, bbox))
                    }
                }
            }
        }
    }

    /// Write back stationary entries no longer matched
    /// stationary => finalized_stationary
    fn finalize_stationary(&mut self, dnn_timestamp: u64) {
        // Find objects no longer stationary
        let moved = self
            .stationary
            .drain_filter(|s| {
                // Check for DNN from corresponding tracker
                s.range_start <= dnn_timestamp
                    && dnn_timestamp < s.range_end
                    // Check how long the stationary was updated
                    && self.stationary_maxage + s.end < dnn_timestamp
            })
            // Assure at least two detections were made
            .filter(|s| s.range_start != s.range_end);

        self.finalized_stationary.extend(moved);
    }

    /// Keeps track of maximum track_id for unique id
    fn update_max_track_id(&mut self, track: &Vec<Bbox>) {
        let new_track_id = track.first().unwrap().track_id.unwrap();
        self.max_track_id = u64::max(self.max_track_id, new_track_id);
    }

    /// Try associating with new detection result
    fn update_dnn(&mut self, dnn_bboxes: Vec<Bbox>) -> Result<(), Box<dyn std::error::Error>> {
        for dnn_timestamp in dnn_bboxes
            .iter()
            .map(|dnn_bbox| dnn_bbox.timestamp.unwrap())
            .unique()
        {
            self.finalize_stationary(dnn_timestamp);

            // Finalize tracks based on current timestamp
            self.finalize_trk(dnn_timestamp)?;
        }

        for dnn_bbox in dnn_bboxes {
            let dnn_timestamp = dnn_bbox.timestamp.unwrap();

            // Write down to CSV file
            self.dnn_writer.serialize(&dnn_bbox)?;

            // Check if DNN bounding box matches any tracks
            // Flag to see if the inferenced box has been matched to any tracks
            let mut matched_flag = false;
            for (_, _, trk) in self
                .tracks
                .iter()
                .filter(|(range_start, range_end, _)| {
                    // Find only the tracks that DNN detection falls between the range
                    *range_start <= dnn_timestamp && dnn_timestamp < *range_end
                })
                .filter(|(_, _, trk)| {
                    // Find only the tracks older than DNN detection
                    trk.first().unwrap().timestamp.unwrap() <= dnn_timestamp
                })
            {
                // Find the track bounding box at the same timestamp
                let trk_bbox = trk
                    .iter()
                    .find(|trk_bbox| trk_bbox.timestamp.unwrap() == dnn_timestamp)
                    .unwrap(); // Should not panic since finalize_trk is called beforehand

                // Scale up one of the bouding box before checking IoU
                let mut trk_bbox = trk_bbox.clone();
                trk_bbox.scale(self.scale_factor);

                // Test IoU and assign class accordingly
                let iou = trk_bbox.iou(&dnn_bbox);
                let trk_id = trk_bbox.track_id.unwrap();
                trace!(
                    "[ASSOC] Detection at {} with track {}  IoU: {}",
                    dnn_timestamp,
                    trk_id,
                    iou
                );
                if iou >= self.moving_iou {
                    let class_id = dnn_bbox.class_id.unwrap();
                    let entry = self.track2class.entry(trk_id).or_insert_with(|| vec![]);
                    trace!(
                        "[ASSOC] Detection at {} was associated to track {}, class {}",
                        dnn_timestamp,
                        trk_id,
                        class_id,
                    );

                    entry.push(class_id);

                    matched_flag = true;
                }
            }

            self.dnns.push_back((matched_flag, dnn_bbox));
        }
        Ok(())
    }

    /// Try associating with new tracking result
    fn update_track(
        &mut self,
        range_start: u64,
        oldest: u64,
        trk: Vec<Bbox>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let range_end = *self.tracker_range.get(&range_start).unwrap();

        // Write down to CSV file
        for bbox in &trk {
            self.track_writer.serialize(bbox)?;
        }

        // Update max track_id value
        self.update_max_track_id(&trk);

        // Look for pending DNNs that coincide any frame in track
        let start_timestamp = trk.first().unwrap().timestamp.unwrap();
        let end_timestamp = trk.last().unwrap().timestamp.unwrap();
        for (matched_flag, dnn_bbox) in self.dnns.iter_mut().filter(|(_, dnn_bbox)| {
            let dnn_timestamp = dnn_bbox.timestamp.unwrap();
            start_timestamp <= dnn_timestamp && dnn_timestamp <= end_timestamp
        }) {
            // Find the track bounding box at the same timestamp
            let dnn_timestamp = dnn_bbox.timestamp.unwrap();
            let trk_bbox = trk
                .iter()
                .find(|trk_bbox| trk_bbox.timestamp.unwrap() == dnn_timestamp)
                .unwrap(); // Should not panic since track is consecutive

            // Scale up one of the bouding box before checking IoU
            let mut trk_bbox = trk_bbox.clone();
            trk_bbox.scale(self.scale_factor);

            // Test IoU and assign class accordingly
            let iou = trk_bbox.iou(&dnn_bbox);
            let trk_id = trk_bbox.track_id.unwrap();
            trace!("[ASSOC] {} Track at {} IoU: {}", trk_id, dnn_timestamp, iou);
            if iou > self.moving_iou {
                let class_id = dnn_bbox.class_id.unwrap();
                let entry = self.track2class.entry(trk_id).or_insert_with(|| vec![]);
                trace!(
                    "[ASSOC] Track at {} was associated to track {}, class {}",
                    dnn_timestamp,
                    trk_id,
                    class_id,
                );
                entry.push(class_id);

                // Mark DNN has been matched
                *matched_flag = true;
            }
        }
        self.tracks.push_back((range_start, range_end, trk));

        // Finalize pending dnn based on oldest timestamp
        self.finalize_dnn(range_start, range_end, oldest);
        Ok(())
    }

    /// Should be called when association server terminates
    fn terminate(mut self) -> Result<(), Box<dyn std::error::Error>> {
        let ranges: Vec<_> = self
            .tracker_range
            .iter()
            .map(|(range_start, range_end)| (*range_start, *range_end))
            .collect();

        // Finalize all pending boxes
        for (range_start, range_end) in ranges {
            self.finalize_trk(range_end)?;
            self.finalize_dnn(range_start, range_end, range_end);
            self.finalize_stationary(range_end);
        }

        // Write down finalized stationary tracks
        let mut new_track_id = self.max_track_id + 1;
        for stationary in &mut self.finalized_stationary {
            // Assign new track ID
            stationary.track_id = Some(new_track_id);
            new_track_id += 1;

            for bbox in stationary.to_vec() {
                self.stationary_writer.serialize(&bbox)?;
            }
        }

        self.track_writer.flush()?;
        self.dnn_writer.flush()?;
        self.assoc_writer.flush()?;
        self.stationary_writer.flush()?;

        Ok(())
    }
}

pub(crate) async fn assoc_server(
    num_tracker: usize,
    track_path: PathBuf,
    dnn_path: PathBuf,
    assoc_path: PathBuf,
    stationary_path: PathBuf,
    mut rx: tokio::sync::mpsc::Receiver<Recieved>,
    barrier: Arc<Barrier>,
    moving_iou: f32,
    stationary_iou: f32,
    stationary_maxage: usize,
    scale_factor: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut assoc = Associator::new(
        track_path,
        dnn_path,
        assoc_path,
        stationary_path,
        moving_iou,
        stationary_iou,
        stationary_maxage,
        scale_factor,
    );
    let mut tracker_range = vec![];

    while let Some(recieved) = rx.recv().await {
        match recieved {
            Recieved::First(range_start) => {
                // Collect range_start of all trackers using barrier
                tracker_range.push(range_start);
                if tracker_range.len() == num_tracker {
                    tracker_range.sort();
                    tracker_range.push(u64::MAX);

                    info!("[ASSOC] Gathered ranges of {:?}", &tracker_range);

                    // Build mapping [start, end) of each tracker
                    (0..num_tracker).for_each(|i| {
                        assoc
                            .tracker_range
                            .insert(tracker_range[i], tracker_range[i + 1]);
                    });
                    debug!("[ASSOC] Releasing first barrier");
                    barrier.wait().await;
                }
            }
            Recieved::Dnn(bboxes) => assoc.update_dnn(bboxes)?,
            Recieved::Track(Frame {
                range_start,
                oldest,
                bboxes,
            }) => assoc.update_track(range_start, oldest, bboxes)?,
        }
    }
    info!("[ASSOC] Terminating");
    assoc.terminate()?;

    Ok(())
}
