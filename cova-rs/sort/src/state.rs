use crate::PrecisionType;
use bbox::Bbox;

pub trait State {
    fn into_z(&self) -> [PrecisionType; 4];
    fn from_x(x: &[PrecisionType]) -> Self;
}

impl State for Bbox {
    fn into_z(&self) -> [PrecisionType; 4] {
        let x = self.left + self.width / 2.;
        let y = self.top + self.height / 2.;
        let r = self.width / self.height;

        [x, y, self.area, r]
    }

    fn from_x(x: &[PrecisionType]) -> Self {
        let r = x[3];
        let s = x[2];
        let y = x[1];
        let x = x[0];

        let width = (s * r).sqrt();
        let height = s / width;
        Bbox::new(x - width / 2., y - width / 2., width, height)
    }
}

// impl State {
//     fn from_bbox(bbox: &Bbox) -> Self {
//         let x = bbox.left + bbox.width / 2.;
//         let y = bbox.top + bbox.height / 2.;
//         let r = bbox.width / bbox.height;

//         State {
//             x,
//             y,
//             s: bbox.area,
//             r,
//             timestamp: bbox.timestamp,
//             class_id: bbox.class_id,
//             track_id: bbox.track_id,
//         }
//     }

//     fn into_vector(&self) -> OVector<PrecisionType, U4> {
//         OVector::<PrecisionType, U4>::new(self.x, self.y, self.s, self.r)
//     }
// }

// impl Into<Bbox> for State {
//     fn into(self) -> Bbox {
//         let width = (self.s * self.r).sqrt();
//         let height = self.s / width;
//         Bbox {
//             left: self.x - width / 2.,
//             top: self.y - width / 2.,
//             width,
//             height,
//             area: self.s,
//             timestamp: self.timestamp,
//             class_id: self.class_id,
//             track_id: self.track_id,
//         }
//     }
// }
