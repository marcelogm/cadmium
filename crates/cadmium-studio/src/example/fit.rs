//! How big the example sheet's canvas is for a brush, and where its two strokes run.

use cadmium_engine::brush::BrushSpec;
use cadmium_engine::engine::{bristle_reach_px, ferrule_radius_for};

use crate::demo::StrokeBox;

/// The example sheet as presented. The editor shows it 312 px wide, so it stays sharp up
/// to a screen scale of about 2.9.
pub const EXAMPLE_SHEET_SIZE: (u32, u32) = (910, 440);
/// The canvas in bristle reaches, at the sheet's aspect. Down: a 2-reach margin, the
/// S-curve's 2, a 2-reach gap, the ramp's 1 and a 2-reach margin; across, both strokes run
/// between 2-reach margins.
pub(super) const CANVAS_REACHES: (f64, f64) = (18.6, 9.0);
pub(super) const MARGIN_REACHES: f64 = 2.0;
const STROKE_WIDTH_REACHES: f64 = CANVAS_REACHES.0 - 2.0 * MARGIN_REACHES;
/// Each stroke's box: its top and its height, in reaches.
const S_CURVE_ROWS: (f64, f64) = (MARGIN_REACHES, 2.0);
const RAMP_ROWS: (f64, f64) = (6.0, 1.0);
/// The largest canvas, at the sheet's aspect: about 1.55 million pixels, about 100 MB of
/// canvas state.
pub(super) const MAX_CANVAS: (u32, u32) = (1792, 866);
const FIT_STEPS: u32 = 40;

/// The brush as the sheet paints it, and the canvas that holds its strokes.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ExampleFit {
    pub(super) brush: BrushSpec,
    pub(super) reach_px: f32,
    pub(super) canvas: (u32, u32),
}

pub(super) fn reach_for(brush: &BrushSpec, size_px: f32) -> f32 {
    bristle_reach_px(ferrule_radius_for(size_px), &brush.head)
}

pub(super) fn canvas_for(reach_px: f32) -> (u32, u32) {
    let side = |reaches: f64| (reaches * f64::from(reach_px)).ceil() as u32;
    (side(CANVAS_REACHES.0), side(CANVAS_REACHES.1))
}

fn fits(canvas: (u32, u32)) -> bool {
    canvas.0 <= MAX_CANVAS.0 && canvas.1 <= MAX_CANVAS.1
}

/// The largest size up to the brush's own whose canvas fits.
fn largest_fitting_size(brush: &BrushSpec) -> f32 {
    let (mut fitting, mut too_big) = (0.0, brush.size_px);
    for _ in 0..FIT_STEPS {
        let size_px = (fitting + too_big) / 2.0;
        if fits(canvas_for(reach_for(brush, size_px))) {
            fitting = size_px;
        } else {
            too_big = size_px;
        }
    }
    fitting
}

impl ExampleFit {
    /// `brush` at its own size if its canvas fits, or scaled down until it does; a
    /// non-finite size becomes the smallest brush size.
    ///
    /// The smallest ferrule radius always fits: its reach is a few pixels for any clamped
    /// head.
    pub(super) fn of(brush: &BrushSpec) -> ExampleFit {
        let mut brush = brush.clone();
        if !brush.size_px.is_finite() {
            brush.size_px = BrushSpec::SIZE_RANGE_PX[0];
        }
        if !fits(canvas_for(reach_for(&brush, brush.size_px))) {
            brush.size_px = largest_fitting_size(&brush);
        }
        let reach_px = reach_for(&brush, brush.size_px);
        debug_assert!(fits(canvas_for(reach_px)), "{brush:?} does not fit");
        ExampleFit {
            canvas: canvas_for(reach_px),
            reach_px,
            brush,
        }
    }

    pub(super) fn s_curve_box(&self) -> StrokeBox {
        self.stroke_box(S_CURVE_ROWS)
    }

    pub(super) fn ramp_box(&self) -> StrokeBox {
        self.stroke_box(RAMP_ROWS)
    }

    fn stroke_box(&self, (top, height): (f64, f64)) -> StrokeBox {
        let reach = f64::from(self.reach_px);
        StrokeBox {
            x: MARGIN_REACHES * reach,
            y: top * reach,
            width: STROKE_WIDTH_REACHES * reach,
            height: height * reach,
        }
    }
}

#[cfg(test)]
mod tests;
