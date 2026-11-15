//! Where the wheel's points are, in the widget's pixels.

use std::f64::consts::TAU;

/// How far inside the edge of the wheel the dots of its colours sit.
pub(crate) const RIM_INSET_PX: f64 = 14.0;

/// The turn (0 at the top, clockwise) and radius (1 on the edge) of a point of a wheel
/// `size_px` across; `None` outside the disc and for a wheel with no size.
pub(crate) fn turn_and_radius_at(x: f64, y: f64, size_px: f64) -> Option<(f32, f32)> {
    let half_px = size_px / 2.0;
    let (dx, dy) = (x - half_px, y - half_px);
    let radius = dx.hypot(dy) / half_px;
    let turn = (dx.atan2(-dy) / TAU).rem_euclid(1.0);
    (radius <= 1.0).then_some((turn as f32, radius as f32))
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
