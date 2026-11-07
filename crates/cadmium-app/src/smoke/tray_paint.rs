//! A smoke step that squeezes a blob of each pigment on the tray and blends two of them, so
//! a screenshot shows paint on the tray's glass.

use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::{Surface, TRAY_HEIGHT, TRAY_WIDTH};

use crate::tools::TrayTool;
use crate::window::CadmiumWindow;

const BLOB_PRESSURE: f32 = 0.8;
const SQUEEZE_STEP_SECONDS: f32 = 0.1;
const SQUEEZE_STEPS: usize = 6;
const BLEND_SAMPLES: usize = 24;

fn sample_at(x: f64, y: f64, time_ms: u32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure: BLOB_PRESSURE,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms,
    }
}

/// Where the `index`th of `count` blobs goes: spaced along the tray's upper third.
fn blob_centre(index: usize, count: usize) -> (f64, f64) {
    let step = f64::from(TRAY_WIDTH) / (count + 1) as f64;
    (step * (index + 1) as f64, f64::from(TRAY_HEIGHT) / 3.0)
}

pub(super) fn paint_tray(window: &CadmiumWindow) {
    let state = window.state();
    let mut state = state.borrow_mut();
    let tray_tool = state.tray_tool();
    let count = state.session.tray.spec.pigment_slots.len();
    state.set_tray_tool(TrayTool::Tube);
    for slot in 0..count {
        let (x, y) = blob_centre(slot, count);
        state.set_tube_slot(slot);
        state.press(Surface::Tray, sample_at(x, y, 0), false);
        for _ in 0..SQUEEZE_STEPS {
            state.advance_hold(SQUEEZE_STEP_SECONDS);
        }
        state.release();
    }
    if count >= 2 {
        // Pull the first blob down and across into the second, so the two mix.
        let (from, to) = (blob_centre(0, count), blob_centre(1, count));
        state.set_tray_tool(TrayTool::Blender);
        state.press(Surface::Tray, sample_at(from.0, from.1, 0), false);
        let samples: Vec<StrokeSample> = (1..=BLEND_SAMPLES)
            .map(|i| {
                let t = i as f64 / BLEND_SAMPLES as f64;
                let sag = f64::from(TRAY_HEIGHT) / 4.0 * (std::f64::consts::PI * t).sin();
                let x = from.0 + (to.0 - from.0) * t;
                sample_at(x, from.1 + sag, i as u32 * 16)
            })
            .collect();
        state.motion(&samples);
        state.release();
    }
    state.set_tray_tool(tray_tool);
}
