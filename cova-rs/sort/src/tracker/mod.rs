use adskalman::{
    CovarianceUpdateMethod, ObservationModel, StateAndCovariance, TransitionModelLinearNoControl,
};
use bbox::Bbox;
use na::dimension::{U4, U7};
use na::{OMatrix, OVector};
use nalgebra as na;
mod linear_observation_model;
use linear_observation_model::PositionObservationModel;
mod motion_model;
use crate::state::State;
use crate::PrecisionType;
use motion_model::ConstantVelocity2DModel;

pub struct KalmanBoxTracker {
    pub id: u64,
    pub start: u64,
    pub seen_ts: Vec<u64>,
    pub last_match: u64,
    pub active: bool,
    transition_model: ConstantVelocity2DModel<PrecisionType>,
    observation_model: PositionObservationModel<PrecisionType>,
    pub history: Vec<Bbox>,
    hits: u64,
    time_since_update: u64,
    pub hit_streaks: u64,
    age: u64,
    previous_estimate: adskalman::StateAndCovariance<PrecisionType, U7>,
    prior: Option<adskalman::StateAndCovariance<PrecisionType, U7>>,
}

impl KalmanBoxTracker {
    pub fn new(id: u64, initial_bbox: &Bbox, start: u64) -> Self {
        let transition_model = motion_model::ConstantVelocity2DModel::new();
        let observation_model = linear_observation_model::PositionObservationModel::new();
        // let kf = KalmanFilterNoControl::new(motion_model, observation_model);
        let [x, y, s, r] = initial_bbox.into_z();
        #[rustfmt::skip]
        let initial_covar = OMatrix::<PrecisionType, U7, U7>::from_vec(vec![
                10.,  0.,  0.,  0.,     0.,     0.,     0.,
                 0., 10.,  0.,  0.,     0.,     0.,     0.,
                 0.,  0., 10.,  0.,     0.,     0.,     0.,
                 0.,  0.,  0., 10.,     0.,     0.,     0.,
                 0.,  0.,  0.,  0., 10000.,     0.,     0.,
                 0.,  0.,  0.,  0.,     0., 10000.,     0.,
                 0.,  0.,  0.,  0.,     0.,     0., 10000.]);

        let initial_estimate = StateAndCovariance::new(
            OVector::<PrecisionType, U7>::from_vec(vec![x, y, s, r, 0., 0., 0.]),
            initial_covar,
        );

        Self {
            id,
            start,
            seen_ts: vec![],
            last_match: start,
            active: false,
            transition_model,
            observation_model,
            history: vec![],
            hits: 0,
            time_since_update: 0,
            hit_streaks: 0,
            age: 0,
            previous_estimate: initial_estimate,
            prior: None,
        }
    }

    pub fn update(self: &mut Self, bbox: Option<&Bbox>) -> Result<(), anyhow::Error> {
        if let Some(bbox) = bbox {
            self.hits += 1;
            self.hit_streaks += 1;

            // FIXME: arbitrary number for now
            if self.hit_streaks >= 5 {
                self.time_since_update = 0;
                self.last_match = bbox.timestamp.unwrap();
            }

            let [x, y, s, r] = bbox.into_z();
            let this_observation = OVector::<PrecisionType, U4>::new(x, y, s, r);
            let prior = self
                .prior
                .as_ref()
                .expect("predict() should be called before update()");
            let this_estimate = self.observation_model.update(
                prior,
                &this_observation,
                CovarianceUpdateMethod::JosephForm,
            )?;
            self.previous_estimate = this_estimate;

            let last = self.history.last_mut().unwrap();
            last.class_id = bbox.class_id;
            last.confidence = bbox.confidence;
        } else {
            self.hit_streaks = 0;
        }
        Ok(())
    }

    pub fn predict(self: &mut Self, ts: u64) -> &Bbox {
        let previous_state = self.previous_estimate.state_mut();
        if previous_state[6] + previous_state[2] <= 0. {
            previous_state[6] = 0.;
        }

        let prior = self.transition_model.predict(&self.previous_estimate);
        let mut bbox = Bbox::from_x(&prior.state().as_slice());
        bbox.track_id = Some(self.id);
        bbox.timestamp = Some(ts);

        self.prior = Some(prior);

        self.age += 1;
        self.time_since_update += 1;
        self.history.push(bbox);
        &self.history.last().unwrap()
    }

    #[inline]
    pub fn should_live(&self, max_age: u64) -> bool {
        self.time_since_update <= max_age
    }

    pub fn check_activate(&mut self, min_hits: u64) {
        if !self.active && self.hit_streaks >= min_hits {
            self.active = true;
        }
    }

    pub fn location_at(&self, ts: u64) -> Option<&Bbox> {
        self.history.iter().find(|bbox| bbox.timestamp == Some(ts))
    }

    pub fn is_seen(&self) -> bool {
        self.seen_ts
            .iter()
            .any(|ts| self.start <= *ts && self.last_match >= *ts)
    }

    pub fn trim_dead_history(&mut self) {
        let mut idx = 0;
        let drop_idx = self.history.len() as u64 - self.time_since_update;
        self.history.retain(|_| {
            idx += 1;
            idx <= drop_idx
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_tracker() {
        let bbox = Bbox::new(0., 0., 2., 2.);
        let mut tracker = KalmanBoxTracker::new(0, &bbox, 0);
        let _prior_bbox = tracker.predict(0);
        let next_bbox = Bbox::new(1., 1., 2., 2.);
        let _new_estimate = tracker.update(Some(&next_bbox));
    }
}
