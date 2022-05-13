use serde::{Deserialize, Serialize};

mod bbox;

pub use crate::bbox::Bbox;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub range_start: u64,
    pub oldest: u64,
    pub bboxes: Vec<Bbox>,
}

impl Frame {
    pub fn ser(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    pub fn de(serialized: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        bincode::deserialize(&serialized[..]).map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_serde() {
        let a = Frame {
            oldest: 0,
            bboxes: vec![Bbox::new(0., 0., 2., 2.)],
        };
        let serialized = a.ser();
        let b = Frame::de(&serialized[..]).unwrap();
        assert_eq!(a, b);
    }
}
