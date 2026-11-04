use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use oleo_engine::canvas::CanvasSpec;
use oleo_engine::document::{DocumentSnapshot, ExportedImage};
use oleo_engine::engine::PaintEngine;
use oleo_engine::gpu::GpuContext;
use oleo_engine::params::{ParamGroup, SimParams};
use oleo_engine::pigments::{MAX_SLOTS, Mixture};
use oleo_engine::present::ViewTransform;
use oleo_engine::render::ViewMode;
use oleo_engine::session::PaintSession;
use oleo_engine::stroke::{StrokeSample, Tool};

use crate::presets::{STARTING_LOAD, starting_recipe};
use crate::tools::UiTool;

const RATE_SMOOTHING: f32 = 0.2;
const MILLISECONDS_PER_SECOND: f32 = 1000.0;
const NEGLIGIBLE_PICKED_FRACTION: f32 = 1e-3;

const EMPTY_MIXTURE: Mixture = Mixture {
    volume_um: 0.0,
    fractions: [0.0; MAX_SLOTS],
};

pub fn clamp_view_mode(mode: ViewMode, slot_count: usize) -> ViewMode {
    match mode {
        ViewMode::Pigment(slot) if usize::from(slot) >= slot_count => match slot_count {
            0 => ViewMode::Color,
            count => ViewMode::Pigment((count.min(MAX_SLOTS) - 1) as u8),
        },
        other => other,
    }
}

pub fn recipe_from_fractions(fractions: &[f32; MAX_SLOTS]) -> Vec<(usize, f32)> {
    fractions
        .iter()
        .enumerate()
        .filter(|(_, fraction)| **fraction > NEGLIGIBLE_PICKED_FRACTION)
        .map(|(slot, fraction)| (slot, *fraction))
        .collect()
}

pub fn recipe_for_palette(
    recipe: &[(usize, f32)],
    old_palette: &[String],
    new_palette: &[String],
) -> Vec<(usize, f32)> {
    recipe
        .iter()
        .filter_map(|(slot, share)| {
            let pigment = old_palette.get(*slot)?;
            let slot = new_palette.iter().position(|id| id == pigment)?;
            Some((slot, *share))
        })
        .collect()
}

pub fn is_render_param(key: &str) -> bool {
    SimParams::specs()
        .iter()
        .any(|spec| spec.key == key && spec.group == ParamGroup::Render)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PenReadout {
    pub pressure: f32,
    pub tilt_x: f32,
    pub tilt_y: f32,
    pub samples_last_frame: u32,
    pub rate_hz: f32,
}

impl PenReadout {
    pub fn observe_hover(&mut self, sample: &StrokeSample) {
        self.record(sample, None);
    }

    pub fn record(&mut self, sample: &StrokeSample, previous_time_ms: Option<u32>) {
        self.pressure = sample.pressure;
        self.tilt_x = sample.tilt_x_deg;
        self.tilt_y = sample.tilt_y_deg;
        let Some(previous) = previous_time_ms else {
            return;
        };
        let interval_ms = sample.time_ms.wrapping_sub(previous);
        if interval_ms == 0 || interval_ms > u32::MAX / 2 {
            return;
        }
        let instant_rate = MILLISECONDS_PER_SECOND / interval_ms as f32;
        self.rate_hz = if self.rate_hz > 0.0 {
            self.rate_hz + (instant_rate - self.rate_hz) * RATE_SMOOTHING
        } else {
            instant_rate
        };
    }
}

pub type SharedState = Rc<RefCell<AppState>>;
pub type Waker = Rc<dyn Fn()>;

pub struct AppState {
    pub gpu: Rc<GpuContext>,
    pub session: PaintSession,
    pub view: ViewTransform,
    pub ui_tool: UiTool,
    pub dirty_since_save: bool,
    pub file: Option<PathBuf>,
    pub pen: PenReadout,
    pub loaded_mixture: Mixture,
    stroking: bool,
    resting_tool: Tool,
    samples_since_frame: u32,
    last_sample_time_ms: Option<u32>,
    waker: Option<Waker>,
}

impl AppState {
    pub fn new(gpu: Rc<GpuContext>, spec: CanvasSpec) -> AppState {
        let mut session = PaintSession::new(&gpu, spec);
        session.brush.recipe = starting_recipe(&session.canvas.spec.pigment_slots);
        session.brush.load = STARTING_LOAD;
        let resting_tool = session.brush.tool;
        let mut state = AppState {
            gpu,
            session,
            view: ViewTransform {
                origin_x: 0.0,
                origin_y: 0.0,
                zoom: 1.0,
            },
            ui_tool: UiTool::default(),
            dirty_since_save: false,
            file: None,
            pen: PenReadout::default(),
            loaded_mixture: EMPTY_MIXTURE,
            stroking: false,
            resting_tool,
            samples_since_frame: 0,
            last_sample_time_ms: None,
            waker: None,
        };
        state.reload_brush();
        state
    }

    pub fn set_waker(&mut self, waker: Waker) {
        self.waker = Some(waker);
    }

    fn wake(&self) {
        if let Some(waker) = &self.waker {
            waker();
        }
    }

    pub fn start_painting(&mut self, spec: CanvasSpec) {
        self.end_stroke();
        let session = PaintSession::new(&self.gpu, spec);
        self.adopt_session(session, None);
    }

    pub fn open_painting(&mut self, snapshot: DocumentSnapshot, path: PathBuf) {
        self.end_stroke();
        let session = snapshot.into_session(&self.gpu);
        self.adopt_session(session, Some(path));
    }

    pub fn capture_document(&mut self) -> DocumentSnapshot {
        DocumentSnapshot::capture(&self.gpu, &mut self.session)
    }

    pub fn render_export(&mut self) -> ExportedImage {
        ExportedImage::render(&self.gpu, &mut self.session)
    }

    fn adopt_session(&mut self, session: PaintSession, file: Option<PathBuf>) {
        let mut brush = self.session.brush.clone();
        brush.tool = self.resting_tool;
        let params = self.session.engine.params().clone();
        let old_palette = self.session.canvas.spec.pigment_slots.clone();
        self.session = session;
        brush.recipe = recipe_for_palette(
            &brush.recipe,
            &old_palette,
            &self.session.canvas.spec.pigment_slots,
        );
        if brush.recipe.is_empty() {
            brush.recipe = starting_recipe(&self.session.canvas.spec.pigment_slots);
        }
        self.session.brush = brush;
        *self.session.engine.params_mut() = params;
        self.reload_brush();
        self.dirty_since_save = false;
        self.file = file;
        self.wake();
    }

    pub fn slot_count(&self) -> usize {
        self.session.canvas.spec.pigment_slots.len()
    }

    pub fn reload_brush(&mut self) {
        self.session.reload_brush(&self.gpu);
        self.loaded_mixture = self.session.brush.reservoir_mixture(self.slot_count());
    }

    pub fn clean_brush(&mut self) {
        self.end_stroke();
        self.session.clean_brush(&self.gpu);
        self.session.brush.recipe.clear();
        self.loaded_mixture = EMPTY_MIXTURE;
    }

    pub fn pick_colour(&mut self, x: f32, y: f32) {
        self.end_stroke();
        let mixture = self.session.pick_color(&self.gpu, x, y);
        self.session.brush.recipe = recipe_from_fractions(&mixture.fractions);
        self.loaded_mixture = mixture;
        self.wake();
    }

    pub fn brush_is_empty(&self) -> bool {
        self.loaded_mixture
            .fractions
            .iter()
            .all(|fraction| *fraction <= 0.0)
    }

    pub fn add_pigment(&mut self, id: &str) -> Result<usize, String> {
        let session = &mut self.session;
        let slot = session.canvas.add_slot(&self.gpu, &session.db, id)?;
        self.dirty_since_save = true;
        self.wake();
        Ok(slot)
    }

    pub fn dry(&mut self) {
        self.end_stroke();
        self.session.dry(&self.gpu);
        self.dirty_since_save = true;
        self.wake();
    }

    pub fn undo(&mut self) -> bool {
        self.end_stroke();
        let undone = self.session.undo(&self.gpu);
        self.dirty_since_save |= undone;
        self.wake();
        undone
    }

    pub fn redo(&mut self) -> bool {
        self.end_stroke();
        let redone = self.session.redo(&self.gpu);
        self.dirty_since_save |= redone;
        self.wake();
        redone
    }

    pub fn can_undo(&self) -> bool {
        self.session.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.session.history.can_redo()
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        let mode = clamp_view_mode(mode, self.slot_count());
        if self.session.view_mode != mode {
            self.session.view_mode = mode;
            self.session.canvas.dirty.mark_all();
            self.wake();
        }
    }

    pub fn set_param(&mut self, key: &str, value: f32) {
        self.session.engine.params_mut().set(key, value);
        if is_render_param(key) {
            self.session.canvas.dirty.mark_all();
            self.wake();
        }
    }

    pub fn reset_params(&mut self) {
        *self.session.engine.params_mut() = SimParams::default();
        self.session.canvas.dirty.mark_all();
        self.wake();
    }

    pub fn set_ui_tool(&mut self, tool: UiTool) {
        self.ui_tool = tool;
        let UiTool::Paint(paint_tool) = tool else {
            return;
        };
        self.resting_tool = paint_tool;
        if !self.stroking {
            self.session.brush.tool = paint_tool;
        }
    }

    pub fn observe_hover_pen(&mut self, sample: &StrokeSample) {
        self.pen.observe_hover(sample);
    }

    pub fn is_stroking(&self) -> bool {
        self.stroking
    }

    pub fn begin_stroke(&mut self, tool: Tool, sample: StrokeSample) {
        self.end_stroke();
        self.session.brush.tool = tool;
        self.session.begin_stroke(&self.gpu, sample);
        self.stroking = true;
        self.last_sample_time_ms = None;
        self.note_samples(std::slice::from_ref(&sample));
        self.wake();
    }

    pub fn add_samples(&mut self, samples: &[StrokeSample]) {
        if !self.stroking || samples.is_empty() {
            return;
        }
        self.session.add_samples(samples);
        self.note_samples(samples);
        self.wake();
    }

    pub fn end_stroke(&mut self) {
        if !self.stroking {
            return;
        }
        self.session.end_stroke(&self.gpu);
        self.stroking = false;
        self.dirty_since_save = true;
        self.wake();
    }

    pub fn encode_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.session.encode_frame(&self.gpu, encoder);
        if !self.stroking {
            self.session.brush.tool = self.resting_tool;
        }
        self.pen.samples_last_frame = std::mem::take(&mut self.samples_since_frame);
    }

    fn note_samples(&mut self, samples: &[StrokeSample]) {
        for sample in samples {
            self.pen.record(sample, self.last_sample_time_ms);
            self.last_sample_time_ms = Some(sample.time_ms);
        }
        self.samples_since_frame += samples.len() as u32;
    }
}

#[cfg(test)]
mod tests {
    use oleo_engine::canvas::StateTexture;

    use super::*;
    use crate::presets::starting_canvas_spec;

    const TEST_CANVAS: (u32, u32) = (512, 256);
    const TEST_BRUSH_DIAMETER_PX: f32 = 16.0;
    const PAINT_ROW_Y: f64 = 60.0;
    const RAG_ROW_Y: f64 = 200.0;
    const STROKE_STEPS: u32 = 20;

    fn painting_state() -> AppState {
        let gpu = Rc::new(GpuContext::new().expect("tests need a Vulkan GPU"));
        let spec = CanvasSpec {
            width: TEST_CANVAS.0,
            height: TEST_CANVAS.1,
            ..starting_canvas_spec()
        };
        let mut state = AppState::new(gpu, spec);
        state.session.brush.geometry.max_diameter_px = TEST_BRUSH_DIAMETER_PX;
        state.reload_brush();
        state
    }

    fn row_samples(y: f64) -> Vec<StrokeSample> {
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

    fn stroke(state: &mut AppState, tool: Tool, samples: &[StrokeSample]) {
        state.begin_stroke(tool, samples[0]);
        state.add_samples(&samples[1..]);
        state.end_stroke();
    }

    fn present_one_frame(state: &mut AppState) {
        let gpu = state.gpu.clone();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        state.encode_frame(&mut encoder);
        gpu.queue.submit([encoder.finish()]);
    }

    fn wet_volume(state: &AppState) -> f32 {
        state
            .session
            .canvas
            .read_f32(&state.gpu, StateTexture::WetVol)
            .iter()
            .sum()
    }

    #[test]
    fn a_stroke_still_queued_when_the_eraser_comes_down_is_painted_with_its_own_brush() {
        let mut state = painting_state();
        stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
        stroke(&mut state, Tool::Rag, &row_samples(RAG_ROW_Y));
        present_one_frame(&mut state);
        assert!(wet_volume(&state) > 0.0);
        assert_eq!(state.session.brush.tool, Tool::Brush);
    }

    #[test]
    fn tool_chosen_mid_stroke_waits_for_the_stroke_to_end() {
        let mut state = painting_state();
        let samples = row_samples(PAINT_ROW_Y);
        state.begin_stroke(Tool::Brush, samples[0]);
        state.set_ui_tool(UiTool::Paint(Tool::Rag));
        state.add_samples(&samples[1..]);
        assert_eq!(state.session.brush.tool, Tool::Brush);
        state.end_stroke();
        present_one_frame(&mut state);
        assert!(wet_volume(&state) > 0.0);
        assert_eq!(state.session.brush.tool, Tool::Rag);
    }

    #[test]
    fn recipe_follows_its_pigments_into_the_new_palette() {
        let old: Vec<String> = ["white", "blue", "red"].map(String::from).to_vec();
        let new: Vec<String> = ["red", "white"].map(String::from).to_vec();
        assert_eq!(
            recipe_for_palette(&[(0, 0.5), (1, 0.2), (2, 0.3)], &old, &new),
            [(1, 0.5), (0, 0.3)]
        );
    }

    #[test]
    fn opened_painting_keeps_the_brush_and_lab_and_remembers_its_file() {
        const PIGMENT: &str = "ultramarine_blue";
        const TUNED_PARAM_VALUE_SHARE: f32 = 0.25;
        let mut current = painting_state();
        let slot = current
            .session
            .canvas
            .spec
            .pigment_slots
            .iter()
            .position(|id| id == PIGMENT)
            .unwrap();
        current.session.brush.recipe = vec![(slot, 1.0)];
        let spec = &SimParams::specs()[0];
        let tuned = spec.min + (spec.max - spec.min) * TUNED_PARAM_VALUE_SHARE;
        current.set_param(spec.key, tuned);
        current.dirty_since_save = true;

        let mut saved = painting_state();
        saved.session.canvas.spec.pigment_slots.reverse();
        let reversed_palette = saved.session.canvas.spec.pigment_slots.clone();
        let snapshot = saved.capture_document();
        let path = PathBuf::from("opened.oleo");
        current.open_painting(snapshot, path.clone());

        assert_eq!(current.session.canvas.spec.pigment_slots, reversed_palette);
        let new_slot = reversed_palette
            .iter()
            .position(|id| id == PIGMENT)
            .unwrap();
        assert_eq!(current.session.brush.recipe, [(new_slot, 1.0)]);
        assert_eq!(
            current.session.brush.geometry.max_diameter_px,
            TEST_BRUSH_DIAMETER_PX
        );
        assert_eq!(current.session.engine.params().get(spec.key), tuned);
        assert_eq!(current.file, Some(path));
        assert!(!current.dirty_since_save);
    }

    fn sample_at(time_ms: u32) -> StrokeSample {
        StrokeSample {
            x: 0.0,
            y: 0.0,
            pressure: 0.5,
            tilt_x_deg: 10.0,
            tilt_y_deg: -20.0,
            time_ms,
        }
    }

    #[test]
    fn pigment_view_beyond_the_palette_shows_the_last_slot() {
        assert_eq!(
            clamp_view_mode(ViewMode::Pigment(6), 4),
            ViewMode::Pigment(3)
        );
        assert_eq!(
            clamp_view_mode(ViewMode::Pigment(2), 4),
            ViewMode::Pigment(2)
        );
        assert_eq!(clamp_view_mode(ViewMode::Pigment(0), 0), ViewMode::Color);
        assert_eq!(clamp_view_mode(ViewMode::Height, 0), ViewMode::Height);
    }

    #[test]
    fn picked_paint_becomes_a_recipe_of_the_pigments_present() {
        let mut fractions = [0.0; MAX_SLOTS];
        fractions[1] = 0.6;
        fractions[4] = 0.3;
        fractions[5] = 1e-5;
        assert_eq!(recipe_from_fractions(&fractions), [(1, 0.6), (4, 0.3)]);
        assert!(recipe_from_fractions(&[0.0; MAX_SLOTS]).is_empty());
    }

    #[test]
    fn only_render_params_repaint_the_whole_canvas() {
        let render = SimParams::specs()
            .iter()
            .find(|spec| spec.group == ParamGroup::Render)
            .unwrap();
        let transfer = SimParams::specs()
            .iter()
            .find(|spec| spec.group == ParamGroup::Transfer)
            .unwrap();
        assert!(is_render_param(render.key));
        assert!(!is_render_param(transfer.key));
        assert!(!is_render_param("no_such_param"));
    }

    #[test]
    fn pen_rate_follows_the_sample_interval() {
        let mut readout = PenReadout::default();
        readout.record(&sample_at(0), None);
        readout.record(&sample_at(5), Some(0));
        assert!((readout.rate_hz - 200.0).abs() < 1e-3);
        assert_eq!(readout.tilt_y, -20.0);
    }

    #[test]
    fn samples_with_the_same_timestamp_leave_the_rate_alone() {
        let mut readout = PenReadout {
            rate_hz: 120.0,
            ..PenReadout::default()
        };
        readout.record(&sample_at(7), Some(7));
        assert_eq!(readout.rate_hz, 120.0);
    }

    #[test]
    fn clock_going_backwards_does_not_spike_the_rate() {
        let mut readout = PenReadout::default();
        readout.record(&sample_at(3), Some(10));
        assert_eq!(readout.rate_hz, 0.0);
    }
}
