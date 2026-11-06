//! How big the studio canvas is for a brush, and where its stroke runs.

use cadmium_engine::brush::BrushSpec;
use cadmium_engine::engine::{bristle_reach_px, ferrule_radius_for};

use crate::demo::StrokeBox;

/// The largest studio canvas, in pixels.
const MAX_CANVAS: (u32, u32) = (2048, 1024);
/// The canvas, in bristle reaches: the stroke box plus a margin on every side.
const CANVAS_REACHES: (f64, f64) = (10.0, 6.0);
const MARGIN_REACHES: f64 = 2.0;
const BOX_REACHES: (f64, f64) = (6.0, 2.0);
const FIT_STEPS: u32 = 40;

/// The brush as the studio paints it, and the canvas that holds its stroke.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StudioFit {
    pub brush: BrushSpec,
    pub reach_px: f32,
    pub canvas: (u32, u32),
}

fn reach_for(brush: &BrushSpec, size_px: f32) -> f32 {
    bristle_reach_px(ferrule_radius_for(size_px), &brush.head)
}

pub(crate) fn canvas_for(reach_px: f32) -> (u32, u32) {
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

impl StudioFit {
    /// `brush` as it is if its canvas fits, or scaled down until it does.
    pub fn of(brush: &BrushSpec) -> StudioFit {
        let mut brush = brush.clone();
        if !fits(canvas_for(reach_for(&brush, brush.size_px))) {
            brush.size_px = largest_fitting_size(&brush);
        }
        let reach_px = reach_for(&brush, brush.size_px);
        StudioFit {
            canvas: canvas_for(reach_px),
            reach_px,
            brush,
        }
    }

    pub fn stroke_box(&self) -> StrokeBox {
        let reach = f64::from(self.reach_px);
        StrokeBox {
            x: MARGIN_REACHES * reach,
            y: MARGIN_REACHES * reach,
            width: BOX_REACHES.0 * reach,
            height: BOX_REACHES.1 * reach,
        }
    }
}

#[cfg(test)]
mod tests;
