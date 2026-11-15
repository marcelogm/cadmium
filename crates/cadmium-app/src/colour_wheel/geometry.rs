//! Where the wheel's points are, in the widget's pixels.

use std::f64::consts::TAU;

/// How far inside the edge of the wheel the dots of its colours sit.
pub(crate) const RIM_INSET_PX: f64 = 14.0;
/// How near the centre of a rim dot a press must fall to grab it.
const RIM_HIT_PX: f64 = 18.0;
/// How far past the edge of the wheel a rim dot must be dragged to come off it.
const REMOVE_REACH_PX: f64 = 20.0;

/// The turn (0 at the top, clockwise) and radius (1 on the edge) of a point of a wheel
/// `size_px` across; `None` outside the disc and for a wheel with no size.
pub(crate) fn turn_and_radius_at(x: f64, y: f64, size_px: f64) -> Option<(f32, f32)> {
    let half_px = size_px / 2.0;
    let radius = (x - half_px).hypot(y - half_px) / half_px;
    (radius <= 1.0).then(|| (turn_at(x, y, size_px), radius as f32))
}

/// The turn of a point, as `turn_and_radius_at` reads it, wherever the point is.
pub(crate) fn turn_at(x: f64, y: f64, size_px: f64) -> f32 {
    let half_px = size_px / 2.0;
    let (dx, dy) = (x - half_px, y - half_px);
    (dx.atan2(-dy) / TAU).rem_euclid(1.0) as f32
}

/// Whether a point is more than `REMOVE_REACH_PX` past the edge of the wheel.
pub(crate) fn is_outside(x: f64, y: f64, size_px: f64) -> bool {
    let half_px = size_px / 2.0;
    (x - half_px).hypot(y - half_px) > half_px + REMOVE_REACH_PX
}

/// The rim dot of `len` nearest the point, if it lies within `RIM_HIT_PX` of it.
pub(crate) fn rim_hit(x: f64, y: f64, len: usize, size_px: f64) -> Option<usize> {
    let distance_px = |position: usize| {
        let (centre_x, centre_y) = rim_centre(position, len, size_px);
        (x - centre_x).hypot(y - centre_y)
    };
    (0..len)
        .map(|position| (position, distance_px(position)))
        .filter(|(_, distance)| *distance < RIM_HIT_PX)
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(position, _)| position)
}

/// The centre of the dot of the circle's colour `position` of `len`.
pub(crate) fn rim_centre(position: usize, len: usize, size_px: f64) -> (f64, f64) {
    let half_px = size_px / 2.0;
    let distance_px = half_px - RIM_INSET_PX;
    let angle = TAU * position as f64 / len.max(1) as f64;
    (
        half_px + distance_px * angle.sin(),
        half_px - distance_px * angle.cos(),
    )
}

#[cfg(test)]
mod tests;
