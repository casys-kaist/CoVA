use bbox::Bbox;
use opencv::imgproc::{connected_components_with_stats, ConnectedComponentsTypes};
use opencv::prelude::*;

pub fn regionprops(
    raw_slice: &[u8],
    width: usize,
    height: usize,
    area_thresh: i32,
) -> opencv::Result<Vec<Bbox>> {
    let _width = width as i32;
    let height = height as i32;

    let input = Mat::from_slice(raw_slice)?;
    let input = input.reshape(1, height)?;

    let mut labels = Mat::default();
    let mut stats = Mat::default();
    let mut centroids = Mat::default();
    let connectivity = 8;
    let ltype = opencv::core::CV_32S;

    let num_objects = connected_components_with_stats(
        &input,
        &mut labels,
        &mut stats,
        &mut centroids,
        connectivity,
        ltype,
    )?;

    const LEFT: i32 = ConnectedComponentsTypes::CC_STAT_LEFT as i32;
    const TOP: i32 = ConnectedComponentsTypes::CC_STAT_TOP as i32;
    const WIDTH: i32 = ConnectedComponentsTypes::CC_STAT_WIDTH as i32;
    const HEIGHT: i32 = ConnectedComponentsTypes::CC_STAT_HEIGHT as i32;
    const AREA: i32 = ConnectedComponentsTypes::CC_STAT_AREA as i32;

    Ok((1..num_objects)
        .filter(|i| stats.at_2d::<i32>(*i, AREA).unwrap() >= &area_thresh)
        .map(|i| {
            let &left = stats.at_2d::<i32>(i, LEFT).unwrap();
            let &top = stats.at_2d::<i32>(i, TOP).unwrap();
            let &width = stats.at_2d::<i32>(i, WIDTH).unwrap();
            let &height = stats.at_2d::<i32>(i, HEIGHT).unwrap();

            Bbox::new(left as f32, top as f32, width as f32, height as f32)
        })
        .collect())
}
