//! Smoke steps on the tray: picking its tool, and mixing two cords with the mixing knife.

use std::ffi::OsString;

use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::Surface;
use gtk::glib;

use crate::LOG_DOMAIN;
use crate::state::AppState;
use crate::tools::TrayTool;
use crate::window::CadmiumWindow;

pub(super) const TRAY_TOOL: &str = "CADMIUM_SMOKE_TRAY_TOOL";
pub(super) const TRAY_MIX: &str = "CADMIUM_SMOKE_TRAY_MIX";

const MIX_PIGMENTS: [&str; 2] = ["cadmium_yellow", "ultramarine_blue"];
const CORD_FROM_X: f64 = 300.0;
const CORD_TO_X: f64 = 700.0;
const CORD_YS: [f64; 2] = [300.0, 330.0];
const CORD_STEPS: u32 = 60;
const CORD_STEP_SECONDS: f32 = 1.0 / 60.0;
const PASS_XS: [f64; 4] = [380.0, 460.0, 540.0, 620.0];
const PASS_TOP_Y: f64 = 240.0;
const PASS_BOTTOM_Y: f64 = 390.0;
const PASS_STEPS: u32 = 40;
const MIXING_PRESSURE: f32 = 0.7;
/// Pen tilts of the passes: upright scrapes paint onto the face, laid flat spreads it.
const PASS_TILTS_DEG: [f32; 4] = [0.0, 60.0, 0.0, 60.0];

pub(super) fn parse_tray_tool(value: OsString) -> Option<TrayTool> {
    let parsed = value.to_str().and_then(TrayTool::from_name);
    if parsed.is_none() {
        glib::g_warning!(
            LOG_DOMAIN,
            "ignoring {TRAY_TOOL}={value:?}: not a tray tool"
        );
    }
    parsed
}

pub(super) fn choose_tray_tool(window: &CadmiumWindow, tool: TrayTool) {
    window.select_tray_tool(tool.name());
}

/// Squeezes a yellow and a blue cord side by side and passes the mixing knife
/// across both a few times, upright and laid flat by turns, leaving it picked.
pub(super) fn mix_on_tray(window: &CadmiumWindow) {
    let canvas = window.canvas();
    let state = canvas.state();
    let mut state = state.borrow_mut();
    let slots = &state.session.canvas.spec.pigment_slots;
    let found = MIX_PIGMENTS.map(|id| slots.iter().position(|slot| slot == id));
    let [Some(first), Some(second)] = found else {
        glib::g_warning!(
            LOG_DOMAIN,
            "smoke tray mix needs {MIX_PIGMENTS:?} on the palette"
        );
        return;
    };
    for (slot, y) in [first, second].into_iter().zip(CORD_YS) {
        squeeze_cord(&mut state, slot, y);
    }
    state.set_tray_tool(TrayTool::MixingKnife);
    for (x, tilt_deg) in PASS_XS.into_iter().zip(PASS_TILTS_DEG) {
        pass_down(&mut state, x, tilt_deg);
    }
    drop(state);
    window.select_tray_tool(TrayTool::MixingKnife.name());
}

fn squeeze_cord(state: &mut AppState, slot: usize, y: f64) {
    state.set_tray_tool(TrayTool::Tube);
    state.set_tube_slot(slot);
    state.press(Surface::Tray, at(CORD_FROM_X, y, 1.0, 0), false);
    for step in 1..=CORD_STEPS {
        let t = f64::from(step) / f64::from(CORD_STEPS);
        let x = CORD_FROM_X + t * (CORD_TO_X - CORD_FROM_X);
        state.motion(&[at(x, y, 1.0, step)]);
        state.advance_hold(CORD_STEP_SECONDS);
    }
    state.release();
}

fn pass_down(state: &mut AppState, x: f64, tilt_deg: f32) {
    let tilted = |sample: StrokeSample| StrokeSample {
        tilt_x_deg: tilt_deg,
        ..sample
    };
    let first = tilted(at(x, PASS_TOP_Y, MIXING_PRESSURE, 0));
    state.press(Surface::Tray, first, false);
    let samples: Vec<_> = (1..=PASS_STEPS)
        .map(|step| {
            let t = f64::from(step) / f64::from(PASS_STEPS);
            let y = PASS_TOP_Y + t * (PASS_BOTTOM_Y - PASS_TOP_Y);
            tilted(at(x, y, MIXING_PRESSURE, step))
        })
        .collect();
    state.motion(&samples);
    state.release();
}

fn at(x: f64, y: f64, pressure: f32, step: u32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms: step * 8,
    }
}

#[cfg(test)]
mod tests;
