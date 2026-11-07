//! How the pen's pressure maps onto the brush's: a curve through the painter's points.
//!
//! The curve is a monotone cubic (Fritsch–Carlson): it passes through every point and never
//! bulges past them, so rising points always give a rising curve.

use serde::{Deserialize, Serialize, Serializer};

/// The most points a curve holds.
pub const MAX_POINTS: usize = 8;
/// The least distance between neighbouring points' inputs.
pub const MIN_GAP: f32 = 0.02;
/// Slack on [`MIN_GAP`] for sums that round a hair short of it.
const GAP_SLACK: f32 = 1e-5;
/// Beyond this, the Fritsch–Carlson tangents are scaled back to keep a segment monotone.
const TANGENT_LIMIT_SQUARED: f32 = 9.0;

/// Points `(input, output)` in the unit square, inputs rising, the first at input 0 and the
/// last at input 1. Kept in a fixed array so the curve is `Copy`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PressureCurve {
    points: [(f32, f32); MAX_POINTS],
    tangents: [f32; MAX_POINTS],
    len: usize,
}

impl Default for PressureCurve {
    fn default() -> Self {
        PressureCurve::linear()
    }
}

impl PressureCurve {
    /// The curve that hands pressure through unchanged.
    pub fn linear() -> PressureCurve {
        PressureCurve::from_points(&[(0.0, 0.0), (1.0, 1.0)]).expect("the linear curve is valid")
    }

    /// The curve through `points`, if they make one: 2 to [`MAX_POINTS`] finite points in the
    /// unit square, the first at input 0, the last at input 1, inputs rising by at least
    /// [`MIN_GAP`].
    pub fn from_points(points: &[(f32, f32)]) -> Option<PressureCurve> {
        let in_unit = |value: f32| (0.0..=1.0).contains(&value);
        let valid = (2..=MAX_POINTS).contains(&points.len())
            && points.iter().all(|&(x, y)| in_unit(x) && in_unit(y))
            && points.first()?.0 == 0.0
            && points.last()?.0 == 1.0
            && points
                .windows(2)
                .all(|pair| pair[1].0 - pair[0].0 >= MIN_GAP - GAP_SLACK);
        if !valid {
            return None;
        }
        let mut stored = [(0.0, 0.0); MAX_POINTS];
        stored[..points.len()].copy_from_slice(points);
        Some(PressureCurve {
            points: stored,
            tangents: tangents(points),
            len: points.len(),
        })
    }

    /// The curve a settings value holds: a list of `[input, output]` pairs.
    pub fn from_json(value: &serde_json::Value) -> Option<PressureCurve> {
        let pairs: Vec<[f32; 2]> = Vec::<[f32; 2]>::deserialize(value).ok()?;
        let points: Vec<(f32, f32)> = pairs.iter().map(|&[x, y]| (x, y)).collect();
        PressureCurve::from_points(&points)
    }

    /// The points, inputs rising.
    pub fn points(&self) -> &[(f32, f32)] {
        &self.points[..self.len]
    }

    /// The brush pressure for pen `pressure`, both in 0..=1.
    pub fn apply(&self, pressure: f32) -> f32 {
        let x = if pressure.is_nan() {
            0.0
        } else {
            pressure.clamp(0.0, 1.0)
        };
        let points = self.points();
        let i = points
            .windows(2)
            .position(|pair| x <= pair[1].0)
            .unwrap_or(points.len() - 2);
        let ((x0, y0), (x1, y1)) = (points[i], points[i + 1]);
        let (width, t) = (x1 - x0, (x - x0) / (x1 - x0));
        let secant = (y1 - y0) / width;
        let (m0, m1) = (self.tangents[i], self.tangents[i + 1]);
        let straight = (m0 - secant).abs() <= f32::EPSILON && (m1 - secant).abs() <= f32::EPSILON;
        // Keeps straight segments, the linear curve above all, exact instead of rounded
        // through the Hermite blend.
        let y = if straight {
            y0 + t * (y1 - y0)
        } else {
            hermite(t, (y0, y1), (m0 * width, m1 * width))
        };
        y.clamp(0.0, 1.0)
    }
}

impl Serialize for PressureCurve {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.points().iter().map(|&(x, y)| [x, y]))
    }
}

/// The cubic Hermite blend at `t` of end values `y0`, `y1` with end slopes `m0`, `m1` (already
/// scaled by the segment's width).
fn hermite(t: f32, (y0, y1): (f32, f32), (m0, m1): (f32, f32)) -> f32 {
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * y0
        + (t3 - 2.0 * t2 + t) * m0
        + (-2.0 * t3 + 3.0 * t2) * y1
        + (t3 - t2) * m1
}

/// The Fritsch–Carlson tangent at each point.
fn tangents(points: &[(f32, f32)]) -> [f32; MAX_POINTS] {
    let n = points.len();
    let mut secants = [0.0; MAX_POINTS];
    for i in 0..n - 1 {
        secants[i] = (points[i + 1].1 - points[i].1) / (points[i + 1].0 - points[i].0);
    }
    let mut m = [0.0; MAX_POINTS];
    m[0] = secants[0];
    m[n - 1] = secants[n - 2];
    for i in 1..n - 1 {
        m[i] = if secants[i - 1] * secants[i] <= 0.0 {
            0.0
        } else {
            (secants[i - 1] + secants[i]) / 2.0
        };
    }
    for (i, &secant) in secants.iter().enumerate().take(n - 1) {
        limit_segment(&mut m, i, secant);
    }
    m
}

/// Flattens or scales back segment `i`'s end tangents so the segment can't overshoot.
fn limit_segment(m: &mut [f32; MAX_POINTS], i: usize, secant: f32) {
    if secant == 0.0 {
        m[i] = 0.0;
        m[i + 1] = 0.0;
        return;
    }
    let (a, b) = (m[i] / secant, m[i + 1] / secant);
    let size = a * a + b * b;
    if size > TANGENT_LIMIT_SQUARED {
        let tau = 3.0 / size.sqrt();
        m[i] = tau * a * secant;
        m[i + 1] = tau * b * secant;
    }
}

#[cfg(test)]
mod tests;
