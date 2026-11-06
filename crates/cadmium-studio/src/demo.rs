//! Scripted strokes that show how a brush behaves.

use cadmium_engine::stroke::StrokeSample;

const SAMPLES: u32 = 90;
const TAP_SAMPLES: u32 = 24;
pub(crate) const SAMPLE_INTERVAL_MS: u32 = 6;
const DEMO_TILT_DEG: f32 = 25.0;
const ZIGZAG_TEETH: f64 = 3.0;

/// A scripted stroke that shows one side of how a brush behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoStroke {
    SCurve,
    RisingLine,
    Zigzag,
    TapAndLift,
}

impl DemoStroke {
    /// Every demo stroke, in the order a picker should list them.
    pub const ALL: [DemoStroke; 4] = [
        DemoStroke::SCurve,
        DemoStroke::RisingLine,
        DemoStroke::Zigzag,
        DemoStroke::TapAndLift,
    ];

    /// The name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            DemoStroke::SCurve => "S-Curve",
            DemoStroke::RisingLine => "Rising Pressure",
            DemoStroke::Zigzag => "Zigzag",
            DemoStroke::TapAndLift => "Tap and Lift",
        }
    }

    fn sample_count(self) -> u32 {
        match self {
            DemoStroke::SCurve | DemoStroke::RisingLine | DemoStroke::Zigzag => SAMPLES,
            DemoStroke::TapAndLift => TAP_SAMPLES,
        }
    }
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
/// Progress `t` runs from 0 to 1 over the stroke's sample count (90, or 24 for the
/// shorter tap), one sample every 6 ms from time 0. Every position lies inside
/// `area` and pressure is clamped to 0..=1.
pub fn samples(stroke: DemoStroke, area: StrokeBox) -> Vec<StrokeSample> {
    let count = stroke.sample_count();
    (0..count)
        .map(|index| {
            let t = f64::from(index) / f64::from(count - 1);
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
        DemoStroke::Zigzag => {
            let tooth = (t * ZIGZAG_TEETH).fract();
            ([t, 0.1 + 0.8 * (1.0 - (2.0 * tooth - 1.0).abs())], 0.7)
        }
        DemoStroke::TapAndLift => ([0.5 + 0.01 * t, 0.5], (t * PI).sin()),
    }
}

#[cfg(test)]
mod tests;
