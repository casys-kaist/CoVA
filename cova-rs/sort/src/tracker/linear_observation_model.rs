use na::allocator::Allocator;
use na::dimension::DimMin;
use na::dimension::{U4, U7};
use na::OMatrix;
use na::{DefaultAllocator, RealField};
use nalgebra as na;

use adskalman::ObservationModel;

use crate::PrecisionType;

// observation model -------

pub struct PositionObservationModel<R: RealField>
where
    DefaultAllocator: Allocator<R, U4, U4>,
    DefaultAllocator: Allocator<R, U7, U4>,
    DefaultAllocator: Allocator<R, U4, U7>,
    DefaultAllocator: Allocator<R, U7, U7>,
    DefaultAllocator: Allocator<R, U4>,
{
    pub observation_matrix: OMatrix<R, U4, U7>,
    pub observation_matrix_transpose: OMatrix<R, U7, U4>,
    pub observation_noise_covariance: OMatrix<R, U4, U4>,
}

impl PositionObservationModel<PrecisionType> {
    #[allow(dead_code)]
    pub fn new() -> Self {
        // Create observation model. We only observe the position.
        // Note that from_vec uses row major
        #[rustfmt::skip]
        let observation_matrix = OMatrix::<PrecisionType,U4,U7>::from_vec(vec![
            1., 0., 0., 0.,
            0., 1., 0., 0.,
            0., 0., 1., 0.,
            0., 0., 0., 1.,
            0., 0., 0., 0.,
            0., 0., 0., 0.,
            0., 0., 0., 0.]);

        #[rustfmt::skip]
        let observation_noise_covariance = OMatrix::<PrecisionType, U4, U4>::new(
            1., 0.,  0.,  0.,
            0., 1.,  0.,  0.,
            0., 0., 10.,  0.,
            0., 0.,  0., 10.);

        Self {
            observation_matrix,
            observation_matrix_transpose: observation_matrix.transpose(),
            observation_noise_covariance,
        }
    }
}

impl ObservationModel<PrecisionType, U7, U4> for PositionObservationModel<PrecisionType>
where
    DefaultAllocator: Allocator<PrecisionType, U7, U7>,
    DefaultAllocator: Allocator<PrecisionType, U4, U7>,
    DefaultAllocator: Allocator<PrecisionType, U7, U4>,
    DefaultAllocator: Allocator<PrecisionType, U4, U4>,
    DefaultAllocator: Allocator<PrecisionType, U7>,
    DefaultAllocator: Allocator<PrecisionType, U4>,
    DefaultAllocator: Allocator<(usize, usize), U4>,
    U4: DimMin<U4, Output = U4>,
{
    fn H(&self) -> &OMatrix<PrecisionType, U4, U7> {
        &self.observation_matrix
    }
    fn HT(&self) -> &OMatrix<PrecisionType, U7, U4> {
        &self.observation_matrix_transpose
    }
    fn R(&self) -> &OMatrix<PrecisionType, U4, U4> {
        &self.observation_noise_covariance
    }
}
