use libc::size_t;

use bbox::Bbox;
use std::slice;

#[no_mangle]
pub extern "C" fn bboxes_new() -> *mut Vec<Bbox> {
    let bboxes = Box::new(Vec::new());
    Box::into_raw(bboxes)
}

#[no_mangle]
pub extern "C" fn bboxes_add(
    ptr: *mut Vec<Bbox>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    timestamp: u64,
    class_id: u32,
    confidence: f32,
) {
    let bboxes = unsafe {
        assert!(!ptr.is_null());
        &mut *ptr
    };

    let mut bbox = Bbox::new(left, top, width, height);
    bbox.timestamp = Some(timestamp);
    bbox.class_id = Some(class_id);
    bbox.confidence = Some(confidence);

    bboxes.push(bbox);
}

#[no_mangle]
pub extern "C" fn bboxes_end(ptr: *mut Vec<Bbox>, out: *mut u8, len: size_t) -> u32 {
    let bboxes = unsafe {
        assert!(!ptr.is_null());
        Box::from_raw(ptr)
    };

    let out = unsafe {
        assert!(!out.is_null());
        slice::from_raw_parts_mut(out, len as usize)
    };

    let serialized = Bbox::serialize_vec(&bboxes);
    let len = serialized.len();
    out[..len].copy_from_slice(&serialized[..]);
    len as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_bboxes_new() {
        let ptr = bboxes_new();
        assert!(!ptr.is_null());
        let bboxes = unsafe { Box::from_raw(ptr) };
        assert_eq!(bboxes.len(), 0);
    }

    #[test]
    fn test_bboxes_add() {
        let ptr = bboxes_new();
        assert!(!ptr.is_null());
        bboxes_add(ptr, 0., 0., 0., 0., 0, 0);
        bboxes_add(ptr, 0., 0., 0., 0., 0, 0);

        let bboxes = unsafe { Box::from_raw(ptr) };
        assert_eq!(bboxes.len(), 2);
        let mut expected = Bbox::new(0., 0., 0., 0.);
        expected.timestamp = Some(0);
        expected.class_id = Some(0);
        assert_eq!(bboxes[0], expected);
        assert_eq!(bboxes[1], expected);
    }

    #[test]
    fn test_bboxes_end() {
        let ptr = bboxes_new();
        assert!(!ptr.is_null());
        bboxes_add(ptr, 0., 0., 0., 0., 0, 0);
        bboxes_add(ptr, 0., 0., 0., 0., 0, 0);

        let mut out = [0; 100];
        let len = bboxes_end(ptr, out.as_mut_ptr(), 100) as usize;

        let mut expected_bbox = Bbox::new(0., 0., 0., 0.);
        expected_bbox.timestamp = Some(0);
        expected_bbox.class_id = Some(0);
        let expected = Bbox::serialize_vec(&vec![expected_bbox.clone(), expected_bbox]);

        assert_eq!(&out[..len], &expected[..]);
    }
}
