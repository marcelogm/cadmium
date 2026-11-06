//! Scripted strokes that show how a brush behaves.

use cadmium_engine::stroke::StrokeSample;

const SAMPLES: u32 = 90;
const SAMPLE_INTERVAL_MS: u32 = 6;
const DEMO_TILT_DEG: f32 = 25.0;

/// A scripted stroke that shows one side of how a brush behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoStroke {
    SCurve,
    RisingLine,
}

/// The canvas area a demo stroke must stay inside, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The pen samples of `stroke`, drawn inside `area`.
///
/// Progress `t` runs from 0 to 1 over `SAMPLES` samples, one every `SAMPLE_INTERVAL_MS` from
/// time 0. Every position lies inside `area` and pressure is clamped to 0..=1.
pub fn samples(stroke: DemoStroke, area: StrokeBox) -> Vec<StrokeSample> {
    (0..SAMPLES)
        .map(|index| {
            let t = f64::from(index) / f64::from(SAMPLES - 1);
            let ([u, v], pressure) = shape(stroke, t);
            StrokeSample {
                x: area.x + u * area.width,
                y: area.y + v * area.height,
                pressure: pressure.clamp(0.0, 1.0) as f32,
                tilt_x_deg: DEMO_TILT_DEG,
                tilt_y_deg: 0.0,
                time_ms: index * SAMPLE_INTERVAL_MS,
            }
        })
        .collect()
}

/// Position in the unit box and pressure at progress `t` in 0..=1.
fn shape(stroke: DemoStroke, t: f64) -> ([f64; 2], f64) {
    use std::f64::consts::{PI, TAU};
    match stroke {
        DemoStroke::SCurve => (
            [t, 0.5 + 0.4 * (t * TAU).sin()],
            0.25 + 0.7 * (t * PI).sin(),
        ),
        DemoStroke::RisingLine => ([t, 0.5], 0.1 + 0.85 * t),
    }
}

#[cfg(test)]
mod tests;
