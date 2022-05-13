#![feature(drain_filter)]

pub type PrecisionType = f32;

mod tracker;

mod state;
use bbox::Bbox;
use linear_assignment::{solver, MatrixSize};
use nalgebra::DMatrix;
use ordered_float::OrderedFloat;
use tracker::KalmanBoxTracker as Tracker;

pub struct Sort {
    pub width: usize,
    pub height: usize,
    pub max_age: u64,
    min_hits: u64,
    iou_threshold: PrecisionType,
    pub trackers: Vec<Tracker>,
    frame_count: u64,
    id_counter: u64,
}

fn linear_assignment(cost_matrix: &DMatrix<OrderedFloat<PrecisionType>>) -> Vec<(usize, usize)> {
    let (n_trackers, n_dets) = cost_matrix.shape();

    let mut target = if n_trackers != n_dets {
        let longer = std::cmp::max(n_trackers, n_dets);
        let extended_size = longer * longer;
        let mut zero_vec = Vec::with_capacity(extended_size);
        for _ in 0..extended_size {
            zero_vec.push(OrderedFloat(0.0));
        }
        let mut extended_costs = DMatrix::from_vec(longer, longer, zero_vec);
        extended_costs
            .slice_mut((0, 0), (n_trackers, n_dets))
            .copy_from(cost_matrix);
        extended_costs
    } else {
        cost_matrix.clone()
    };
    let size = MatrixSize {
        rows: target.nrows(),
        columns: target.ncols(),
    };

    let max_weight = OrderedFloat(2.);

    let edges = solver(&mut target, &size);
    edges
        .into_iter()
        .filter(|(i, j)| i < &n_trackers && j < &n_dets)
        .filter(|(i, j)| cost_matrix[(*i, *j)] != max_weight)
        .collect()
}

impl Sort {
    pub fn new(
        width: usize,
        height: usize,
        max_age: u64,
        min_hits: u64,
        iou_threshold: PrecisionType,
    ) -> Self {
        Sort {
            width,
            height,
            max_age,
            min_hits,
            iou_threshold,
            trackers: vec![],
            frame_count: 0,
            id_counter: 0,
        }
    }

    /// Generate cost matrix of negated IoU
    ///
    /// row is mapped to predictions and column is mapped to detections
    fn generate_iou_matrix(
        preds: &Vec<Bbox>,
        dets: &Vec<Bbox>,
    ) -> DMatrix<OrderedFloat<PrecisionType>> {
        let nrows = preds.len();
        let ncols = dets.len();
        DMatrix::from_iterator(
            nrows,
            ncols,
            dets.iter()
                .flat_map(|d| preds.iter().map(move |p| -OrderedFloat(d.iou(p)))),
        )
    }

    /// Match detections to existing trackers
    /// and returns (tracker, detection) match
    /// Solve assignment problem on the cost matrix
    fn match_dets(
        &self,
        preds: &Vec<Bbox>,
        dets: &Vec<Bbox>,
    ) -> Result<Vec<(usize, usize)>, anyhow::Error> {
        let n_preds = preds.len();
        let n_dets = dets.len();

        Ok(if n_preds != 0 && n_dets != 0 {
            let mut cost_matrix = Sort::generate_iou_matrix(preds, dets);
            for (i, mut row) in cost_matrix.row_iter_mut().enumerate() {
                let trk = &self.trackers[i];
                // Make IoU matrix to prefer active tracks
                let weight = if trk.active { 1. } else { 2. };
                row.iter_mut().for_each(|x| *x += weight);
            }

            let assigned = linear_assignment(&cost_matrix);

            assigned
                .into_iter()
                .filter(|(i, j)| {
                    let threshold = if self.trackers[*i].active {
                        OrderedFloat(1. - self.iou_threshold)
                    } else {
                        OrderedFloat(2. - self.iou_threshold)
                    };
                    cost_matrix[(*i, *j)] <= threshold
                })
                .collect()
        } else {
            vec![]
        })
    }

    /// Perform tracking on the new frame and returns the least PTS of unseen object
    pub fn update(&mut self, mut dets: Vec<Bbox>, pts: u64) -> Result<Vec<Tracker>, anyhow::Error> {
        self.frame_count += 1;
        let n_dets = dets.len();

        let preds = self
            .trackers
            .iter_mut()
            .map(|trk| trk.predict(pts).clone())
            .collect();
        let mut matches = self.match_dets(&preds, &dets)?;

        // Index of unmatched detections
        let unmatched_det_idx: Vec<usize> = (0..n_dets)
            .filter(|i| matches.iter().all(|(_, j)| i != j))
            .collect();

        for (i, trk) in self.trackers.iter_mut().enumerate() {
            let det = matches
                .iter_mut()
                .find(|(trk_idx, _)| trk_idx == &i)
                .map(|(_, det_idx)| {
                    dets[*det_idx].timestamp = Some(pts);
                    &dets[*det_idx]
                });
            trk.update(det)?;
        }

        // Activate tracks older than self.min_hits
        let min_hits = self.min_hits;
        self.trackers
            .iter_mut()
            .for_each(|trk| trk.check_activate(min_hits));

        let max_age = self.max_age;

        let dead_tracks = self
            .trackers
            .drain_filter(|trk| !trk.should_live(max_age))
            .filter(|trk| trk.active)
            .map(|mut trk| {
                trk.trim_dead_history();
                trk
            })
            .collect();

        // Initialize new trackers for unmatched detections
        for i in unmatched_det_idx {
            let tracker = Tracker::new(self.id_counter, &dets[i], pts);
            self.id_counter += 1;
            self.trackers.push(tracker);
        }

        Ok(dead_tracks)
    }

    pub fn mark_seen(&mut self, ts: u64) {
        self.trackers
            .iter_mut()
            .for_each(|trk| trk.seen_ts.push(ts));
    }

    pub fn mark_active_seen(&mut self, ts: u64) {
        self.trackers
            .iter_mut()
            .filter(|trk| trk.active)
            .filter(|trk| trk.start <= ts)
            .for_each(|trk| trk.seen_ts.push(ts));
    }

    pub fn any_valid(&self) -> bool {
        self.trackers.iter().any(|trk| trk.active)
    }

    pub fn finalize(&mut self) -> Vec<Tracker> {
        let min_hits = self.min_hits as usize;
        self.trackers
            .drain_filter(|trk| trk.active)
            .filter(|trk| trk.history.len() > min_hits)
            .collect()
    }
}

impl Default for Sort {
    fn default() -> Self {
        let width = 80 * 2;
        let height = 45 * 2;
        let max_age = 3;
        let min_hits = 3;
        let iou_threshold = 0.2;
        Sort::new(width, height, max_age, min_hits, iou_threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_sort() -> anyhow::Result<()> {
        let mut sort: Sort = Default::default();
        let dets = vec![Bbox::new(0., 0., 2., 2.), Bbox::new(1., 1., 2., 2.)];
        sort.update(dets.clone(), 0)?;

        // Check fields
        assert_eq!(sort.frame_count, 1);

        // Check trackers
        assert_eq!(sort.trackers.len(), 2);
        let states = sort.trackers.iter().map(|trk| trk.get_state());
        for (i, state) in states.enumerate() {
            assert_eq!(state, dets[i]);
        }
        Ok(())
    }

    #[test]
    fn test_obeservation_model() -> anyhow::Result<()> {
        let mut sort: Sort = Default::default();
        let dets = vec![Bbox::new(0., 0., 2., 2.), Bbox::new(1., 1., 2., 2.)];
        // Initialize new trackers
        sort.update(dets.clone(), 0)?;
        assert_eq!(sort.trackers.len(), 2);

        sort.trackers.iter_mut().for_each(|trk| {
            trk.predict(0);
        });
        // Check trackers
        assert_eq!(sort.trackers.len(), 2);
        let mut states: Vec<_> = sort
            .trackers
            .iter()
            .map(|trk| trk.history.last().unwrap().clone())
            .collect();

        for (i, state) in states.iter_mut().enumerate() {
            state.track_id = None;
            state.timestamp = None;
            assert_eq!(*state, dets[i]);
        }
        Ok(())
    }

    #[test]
    fn test_generate_iou_matrix() {
        let dets = vec![Bbox::new(0., 0., 2., 2.), Bbox::new(1., 1., 1., 1.)];
        let preds = vec![Bbox::new(1., 1., 1., 1.)];

        let expected = DMatrix::from_vec(1, 2, vec![OrderedFloat(-0.25), OrderedFloat(-1.)]);

        assert_eq!(Sort::generate_iou_matrix(&preds, &dets), expected);
    }

    #[test]
    fn test_linear_assignment_5x5() {
        #[rustfmt::skip]
        let cost_matrix: DMatrix<OrderedFloat<PrecisionType>>  = DMatrix::from_vec(
            5, 5,
            vec! [
            -1.,  0., 0.,  0., 0.,
             0., -1., 0.,  0., 0.,
             0.,  0., 0., -1., 0.,
             0.,  0., 0.,  0., 0.,
             0.,  0., 0.,  0., 0.,
            ].into_iter().map(|x| OrderedFloat(2. + x)).collect::<Vec<OrderedFloat<PrecisionType>>>()
        );

        let mut result = linear_assignment(&cost_matrix);
        #[rustfmt::skip]
        let mut expected = vec![
            (0, 0), (1, 1), (3, 2)
        ];

        result.sort();
        expected.sort();
        assert_eq!(result, expected);
    }
    #[test]
    fn test_linear_assignment_2x3() {
        #[rustfmt::skip]
        let cost_matrix = DMatrix::from_vec(
            2, 3,
            vec![
            -1.,  0.,
             0.,  0.,
             0., -1.,
            ].into_iter().map(|x| OrderedFloat(1. + x)).collect::<Vec<OrderedFloat<PrecisionType>>>()
        );

        let mut result = linear_assignment(&cost_matrix);
        #[rustfmt::skip]
        let mut expected = vec![
            (0, 0), (1, 2)
        ];

        result.sort();
        expected.sort();
        assert_eq!(result, expected);
    }
    #[test]
    fn test_linear_assignment_3x2() {
        #[rustfmt::skip]
        let cost_matrix = DMatrix::from_vec(
            3, 2,
            vec! [
            -1., 0.,  0.,
             0., 0., -1.,
            ].into_iter().map(|x| OrderedFloat(1. + x)).collect::<Vec<OrderedFloat<PrecisionType>>>()
        );

        let mut result = linear_assignment(&cost_matrix);
        #[rustfmt::skip]
        let mut expected = vec![
            (0, 0), (2, 1)
        ];

        result.sort();
        expected.sort();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_linear_assignment_9x8() {
        #[rustfmt::skip]
        let cost_matrix = DMatrix::from_vec(
            9, 8,
            vec! [
            -1.,  0.,  0., 0.,  0.,  0.,  0.,  0.,  0.,
             0., -1.,  0., 0.,  0.,  0.,  0.,  0.,  0.,
             0.,  0., -1., 0.,  0.,  0.,  0.,  0.,  0.,
             0.,  0.,  0., 0., -1.,  0.,  0.,  0.,  0.,
             0.,  0.,  0., 0.,  0., -1.,  0.,  0.,  0.,
             0.,  0.,  0., 0.,  0.,  0., -1.,  0.,  0.,
             0.,  0.,  0., 0.,  0.,  0.,  0., -1.,  0.,
             0.,  0.,  0., 0.,  0.,  0.,  0.,  0., -1.,
            ].into_iter().map(|x| OrderedFloat(1. + x)).collect::<Vec<OrderedFloat<PrecisionType>>>()
        );

        let mut result = linear_assignment(&cost_matrix);
        #[rustfmt::skip]
        let mut expected = vec![
            (0, 0), (1, 1), (2, 2), (4, 3),
            (5, 4), (6, 5), (7, 6), (8, 7),
        ];

        result.sort();
        expected.sort();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_match_dets() -> anyhow::Result<()> {
        let mut sort: Sort = Default::default();
        let first_dets = vec![Bbox::new(0., 0., 4., 4.), Bbox::new(1., 1., 4., 4.)];
        // Initialize new trackers
        sort.update(first_dets, 0)?;
        assert_eq!(sort.trackers.len(), 2);
        // Perform prediction
        let preds = sort
            .trackers
            .iter_mut()
            .map(|trk| trk.predict(0).clone())
            .collect();

        let second_dets = vec![
            Bbox::new(1., 1., 4., 4.),
            Bbox::new(2., 2., 4., 4.),
            Bbox::new(3., 3., 4., 4.),
        ];
        // Generate with exact same bboxes
        let matches = sort.match_dets(&preds, &second_dets)?;
        let expected = vec![(1, 0)];
        assert_eq!(matches, expected);
        Ok(())
    }
}
