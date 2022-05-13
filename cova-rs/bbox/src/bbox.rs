use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bbox {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub area: f32,
    pub track_id: Option<u64>,
    pub timestamp: Option<u64>,
    pub class_id: Option<u32>,
    pub confidence: Option<f32>,
}

impl Bbox {
    pub fn new(left: f32, top: f32, width: f32, height: f32) -> Self {
        Self {
            left,
            top,
            width,
            height,
            area: width * height,
            track_id: None,
            timestamp: None,
            class_id: None,
            confidence: None,
        }
    }

    /// Return coordinate of the Bbox in the form of ((x1, y1), (x2, y2))
    pub fn coordinate(&self) -> ((f32, f32), (f32, f32)) {
        (
            (self.left, self.top),
            (self.left + self.width, self.top + self.height),
        )
    }

    pub fn iou(&self, target: &Bbox) -> f32 {
        let ((s_x1, s_y1), (s_x2, s_y2)) = self.coordinate();
        let ((t_x1, t_y1), (t_x2, t_y2)) = target.coordinate();

        let x_left = f32::max(s_x1, t_x1);
        let y_top = f32::max(s_y1, t_y1);
        let x_right = f32::min(s_x2, t_x2);
        let y_bottom = f32::min(s_y2, t_y2);

        if x_right <= x_left || y_bottom <= y_top {
            0.
        } else {
            let intersect_area = (x_right - x_left) * (y_bottom - y_top);
            let union_area = self.area + target.area - intersect_area;

            intersect_area / union_area
        }
    }

    pub fn scale_dim(&mut self, scale: f32) {
        if scale == 1. {
            return;
        }
        self.left *= scale;
        self.top *= scale;
        self.width *= scale;
        self.height *= scale;
        self.area *= scale * scale;
    }

    pub fn scale(&mut self, scale: f32) {
        if scale == 1. {
            return;
        }
        // Centroid should not move
        let x = self.left + self.width / 2.;
        let y = self.top + self.height / 2.;

        self.width *= scale;
        self.height *= scale;
        self.left = x - self.width / 2.;
        self.top = y - self.height / 2.;
        self.area *= scale * scale;
    }

    pub fn serialize_vec(bboxes: &Vec<Bbox>) -> Vec<u8> {
        bincode::serialize(bboxes).unwrap()
    }

    pub fn deserialize_vec(serialized: &[u8]) -> Result<Vec<Bbox>, Box<dyn std::error::Error>> {
        bincode::deserialize(&serialized[..]).map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iou_same() {
        let a = Bbox::new(0., 0., 2., 2.);
        let b = Bbox::new(0., 0., 2., 2.);

        let expected: f32 = 1.;
        assert_eq!(a.iou(&b), expected);
    }

    #[test]
    fn test_iou_quarter() {
        let a = Bbox::new(0., 0., 2., 2.);
        let b = Bbox::new(1., 1., 2., 2.);

        let expected: f32 = 1. / 7.;
        assert_eq!(a.iou(&b), expected);
    }

    #[test]
    fn test_iou_none() {
        let a = Bbox::new(0., 0., 2., 2.);
        let b = Bbox::new(2., 2., 2., 2.);

        let expected: f32 = 0.;
        assert_eq!(a.iou(&b), expected);
    }

    #[test]
    fn test_serde_vec() {
        let a = vec![Bbox::new(0., 0., 2., 2.)];
        let serialized = Bbox::serialize_vec(&a);
        let b = Bbox::deserialize_vec(&serialized[..]).unwrap();
        assert_eq!(a, b);
    }
}
