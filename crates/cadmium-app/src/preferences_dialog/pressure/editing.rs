//! What a press, a drag and a double-click do to the curve's points, in unit coordinates.

use crate::pressure_curve::{MAX_POINTS, MIN_GAP, PressureCurve};

/// The point within `radius` of `at`, the nearest if several are.
pub(super) fn point_near(curve: &PressureCurve, at: (f32, f32), radius: f32) -> Option<usize> {
    curve
        .points()
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| (i, (x - at.0).hypot(y - at.1)))
        .filter(|&(_, distance)| distance <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// What a press at `at` takes hold of: the point there, or a new one added there. `None`
/// when there is no room for a new point.
pub(super) fn grab(
    curve: &PressureCurve,
    at: (f32, f32),
    radius: f32,
) -> Option<(PressureCurve, usize)> {
    match point_near(curve, at, radius) {
        Some(index) => Some((*curve, index)),
        None => insert(curve, at),
    }
}

fn insert(curve: &PressureCurve, (x, y): (f32, f32)) -> Option<(PressureCurve, usize)> {
    let points = curve.points();
    if points.len() >= MAX_POINTS {
        return None;
    }
    let index = points.iter().position(|point| point.0 > x)?;
    let mut next = points.to_vec();
    next.insert(index, (x, y.clamp(0.0, 1.0)));
    PressureCurve::from_points(&next).map(|added| (added, index))
}

/// The curve with point `index` moved towards `to`: the end points keep their input, an
/// inner point stays [`MIN_GAP`] clear of its neighbours.
pub(super) fn drag(curve: &PressureCurve, index: usize, (x, y): (f32, f32)) -> PressureCurve {
    let points = curve.points();
    if index >= points.len() {
        return *curve;
    }
    let last = points.len() - 1;
    let x = match index {
        0 => 0.0,
        i if i == last => 1.0,
        // `max` then `min` rather than `clamp`, which panics if rounding crosses the bounds.
        i => x
            .max(points[i - 1].0 + MIN_GAP)
            .min(points[i + 1].0 - MIN_GAP),
    };
    let mut next = points.to_vec();
    next[index] = (x, y.clamp(0.0, 1.0));
    PressureCurve::from_points(&next).unwrap_or(*curve)
}

/// The curve without inner point `index`; the end points stay.
pub(super) fn remove(curve: &PressureCurve, index: usize) -> PressureCurve {
    let points = curve.points();
    if index == 0 || index + 1 >= points.len() {
        return *curve;
    }
    let mut next = points.to_vec();
    next.remove(index);
    PressureCurve::from_points(&next).unwrap_or(*curve)
}

#[cfg(test)]
mod tests;
