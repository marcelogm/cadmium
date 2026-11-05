use std::cell::Cell;
use std::rc::Rc;

use cadmium_engine::canvas::{CanvasSpec, PaintTotals, StateTexture};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;

use super::AppState;
use super::clock::Clock;
use crate::presets::starting_canvas_spec;
use crate::tools::UiTool;

const TEST_CANVAS: (u32, u32) = (512, 256);

pub(crate) const TEST_BRUSH_DIAMETER_PX: f32 = 16.0;
pub(super) const PAINT_ROW_Y: f64 = 60.0;
pub(super) const RAG_ROW_Y: f64 = 200.0;
const STROKE_STEPS: u32 = 20;

pub(crate) fn painting_state() -> AppState {
    let gpu = Rc::new(GpuContext::new().expect("tests need a Vulkan GPU"));
    let spec = CanvasSpec {
        width: TEST_CANVAS.0,
        height: TEST_CANVAS.1,
        ..starting_canvas_spec()
    };
    let mut state = AppState::new(gpu, spec).unwrap();
    state.session.brush.geometry.max_diameter_px = TEST_BRUSH_DIAMETER_PX;
    state.reload_brush();
    state
}

pub(super) fn row_samples(y: f64) -> Vec<StrokeSample> {
    (0..STROKE_STEPS)
        .map(|step| StrokeSample {
            x: 100.0 + f64::from(step) * 10.0,
            y,
            pressure: 1.0,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            time_ms: step * 8,
        })
        .collect()
}

pub(super) fn stroke(state: &mut AppState, tool: Tool, samples: &[StrokeSample]) {
    state.begin_stroke(tool, samples[0]);
    state.add_samples(&samples[1..]);
    state.release();
}

pub(super) fn present_one_frame(state: &mut AppState) {
    let gpu = state.gpu.clone();
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    state.encode_frame(&mut encoder);
    gpu.queue.submit([encoder.finish()]);
}

pub(super) fn wet_volume(state: &AppState) -> f32 {
    state
        .session
        .canvas
        .read_f32(&state.gpu, StateTexture::WetVol)
        .unwrap()
        .iter()
        .sum()
}

pub(super) const WHITE_SLOT: usize = 0;

pub(super) const ULTRAMARINE_SLOT: usize = 4;

pub(super) const RESERVE_TOLERANCE: f64 = 1e-3;

pub(super) fn brush_totals(state: &AppState) -> PaintTotals {
    state.session.engine.brush_totals(&state.gpu).unwrap()
}

pub(super) fn reserve_volume(state: &AppState) -> f64 {
    state
        .session
        .engine
        .reserve_totals(&state.gpu)
        .unwrap()
        .volume
}

pub(super) fn knife_totals(state: &AppState) -> PaintTotals {
    state.session.engine.knife_totals(&state.gpu).unwrap()
}

pub(super) fn holding_the_knife() -> AppState {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Knife));
    state
}

pub(super) fn sample_at(x: f64, y: f64, pressure: f32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms: 0,
    }
}

pub(super) fn present_one_tray_frame(state: &mut AppState) {
    let gpu = state.gpu.clone();
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    state.encode_tray_frame(&mut encoder);
    gpu.queue.submit([encoder.finish()]);
}

pub(super) fn squeeze_blob_on_tray(state: &mut AppState, slot: usize) {
    state.set_ui_tool(UiTool::Tube);
    state.set_tube_slot(slot);
    state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
    state.advance_hold(1.0);
    state.release();
}

pub(super) fn settle_both_surfaces(state: &mut AppState) {
    present_one_frame(state);
    present_one_tray_frame(state);
    assert!(!state.session.has_pending_work());
    assert!(!state.session.tray_has_pending_work());
}

#[derive(Clone)]
pub(super) struct ScriptedClock {
    now_ms: Rc<Cell<f64>>,
}

impl ScriptedClock {
    pub(super) fn starting_at(now_ms: f64) -> ScriptedClock {
        ScriptedClock {
            now_ms: Rc::new(Cell::new(now_ms)),
        }
    }

    pub(super) fn advance(&self, elapsed_ms: f64) {
        self.now_ms.set(self.now_ms.get() + elapsed_ms);
    }
}

impl Clock for ScriptedClock {
    fn now_ms(&self) -> f64 {
        self.now_ms.get()
    }
}

pub(super) fn scripted_painting_state(clock: &ScriptedClock) -> AppState {
    let mut state = painting_state();
    state.use_clock(Box::new(clock.clone()));
    state
}
