use std::cell::RefCell;
use std::f32::consts::PI;
use std::path::PathBuf;
use std::rc::Rc;

use oleo_engine::brush::{BrushShape, FULL_RESERVOIR_VOLUME_UM};
use oleo_engine::canvas::CanvasSpec;
use oleo_engine::document::{DocumentSnapshot, ExportedImage};
use oleo_engine::engine::{PaintEngine, TIP_MARGIN, TIP_SIZE};
use oleo_engine::gpu::GpuContext;
use oleo_engine::params::{ParamGroup, SimParams};
use oleo_engine::pigments::{MAX_SLOTS, Mixture};
use oleo_engine::present::ViewTransform;
use oleo_engine::render::ViewMode;
use oleo_engine::session::PaintSession;
use oleo_engine::stroke::{StrokeSample, Tool};

use crate::presets::{STARTING_LOAD, starting_recipe};
use crate::tools::UiTool;
use crate::writes::{EditCounter, EditMark};

const RATE_SMOOTHING: f32 = 0.2;
const MILLISECONDS_PER_SECOND: f32 = 1000.0;
const NEGLIGIBLE_PICKED_FRACTION: f32 = 1e-3;
pub const FULL_DIP: f32 = 1.0;
pub const PARTIAL_DIP: f32 = 0.25;
const MIN_TIP_RADIUS_PX: f32 = 0.5;
const RESERVE_FACTOR_KEY: &str = "reserve_factor";

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

pub fn mixture_is_empty(mixture: &Mixture) -> bool {
    mixture.volume_um <= 0.0 || mixture.fractions.iter().all(|fraction| *fraction <= 0.0)
}

pub fn full_reserve_capacity(reserve_factor: f32, diameter_px: f32) -> f32 {
    let max_radius = (TIP_SIZE - TIP_MARGIN) as f32 / 2.0;
    let radius = (diameter_px / 2.0).clamp(MIN_TIP_RADIUS_PX, max_radius);
    reserve_factor * FULL_RESERVOIR_VOLUME_UM * PI * radius * radius
}

pub fn reserve_level(volume: f32, capacity: f32) -> f32 {
    if capacity > 0.0 && volume.is_finite() {
        (volume / capacity).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PigmentClick {
    Choose,
    Mix,
    CleanAndLoad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PigmentDip {
    CleanAndLoad,
    DipAgain,
    AddOnTop(usize),
}

pub fn is_only_pigment(recipe: &[(usize, f32)], slot: usize) -> bool {
    let mut present = recipe.iter().filter(|(_, parts)| *parts > 0.0).peekable();
    present.peek().is_some() && present.all(|(chosen, _)| *chosen == slot)
}

pub fn recipe_after_mixing(recipe: &[(usize, f32)], slot: usize) -> Vec<(usize, f32)> {
    let total: f32 = recipe.iter().map(|(_, parts)| parts.max(0.0)).sum();
    if total <= 0.0 {
        return vec![(slot, 1.0)];
    }
    let added = PARTIAL_DIP * total;
    let mut recipe = recipe.to_vec();
    match recipe.iter_mut().find(|(chosen, _)| *chosen == slot) {
        Some((_, parts)) => *parts += added,
        None => recipe.push((slot, added)),
    }
    recipe
}

pub fn plan_pigment_dip(
    recipe: &[(usize, f32)],
    slot: usize,
    click: PigmentClick,
) -> (Vec<(usize, f32)>, PigmentDip) {
    match click {
        PigmentClick::Mix => (
            recipe_after_mixing(recipe, slot),
            PigmentDip::AddOnTop(slot),
        ),
        PigmentClick::Choose if is_only_pigment(recipe, slot) => {
            (recipe.to_vec(), PigmentDip::DipAgain)
        }
        PigmentClick::Choose | PigmentClick::CleanAndLoad => {
            (vec![(slot, 1.0)], PigmentDip::CleanAndLoad)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DryOutcome {
    NothingWet,
    Dried { undoable: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokePreparation {
    None,
    RestoreReserve,
    CleanAndLoad,
}

pub fn stroke_preparation(
    tool: Tool,
    reload_each_stroke: bool,
    clean_each_stroke: bool,
) -> StrokePreparation {
    match tool {
        Tool::Brush if clean_each_stroke => StrokePreparation::CleanAndLoad,
        Tool::Brush if reload_each_stroke => StrokePreparation::RestoreReserve,
        _ => StrokePreparation::None,
    }
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
    edits: EditCounter,
    pub file: Option<PathBuf>,
    pub pen: PenReadout,
    pub tip_mixture: Mixture,
    pub reload_each_stroke: bool,
    pub clean_each_stroke: bool,
    reserve_volume: f32,
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
            edits: EditCounter::default(),
            file: None,
            pen: PenReadout::default(),
            tip_mixture: EMPTY_MIXTURE,
            reload_each_stroke: true,
            clean_each_stroke: false,
            reserve_volume: 0.0,
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

    pub fn has_unsaved_edits(&self) -> bool {
        self.edits.has_unsaved_edits()
    }

    pub fn edit_mark(&self) -> EditMark {
        self.edits.mark()
    }

    pub fn saved_to(&mut self, path: PathBuf, mark: EditMark) {
        self.edits.saved_at(mark);
        self.file = Some(path);
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
        self.edits.forget_unsaved();
        self.file = file;
        self.wake();
    }

    pub fn slot_count(&self) -> usize {
        self.session.canvas.spec.pigment_slots.len()
    }

    pub fn palette_mixture(&self) -> Mixture {
        self.session.brush.reservoir_mixture(self.slot_count())
    }

    pub fn reserve_level(&self) -> f32 {
        let capacity = full_reserve_capacity(
            self.session.engine.params().get(RESERVE_FACTOR_KEY),
            self.session.brush.geometry.max_diameter_px,
        );
        reserve_level(self.reserve_volume, capacity)
    }

    pub fn brush_is_clean(&self) -> bool {
        mixture_is_empty(&self.tip_mixture) && self.reserve_volume <= 0.0
    }

    pub fn reload_brush(&mut self) {
        self.end_stroke();
        self.session.reload_brush(&self.gpu);
        self.measure_brush();
    }

    pub fn clean_brush(&mut self) {
        self.end_stroke();
        self.session.clean_brush(&self.gpu);
        self.measure_brush();
        self.wake();
    }

    pub fn choose_pigment(&mut self, slot: usize, click: PigmentClick) {
        self.end_stroke();
        let (recipe, dip) = plan_pigment_dip(&self.session.brush.recipe, slot, click);
        self.session.brush.recipe = recipe;
        match dip {
            PigmentDip::CleanAndLoad => self.session.reload_brush(&self.gpu),
            PigmentDip::DipAgain => {
                let mixture = self.palette_mixture();
                self.session.dip(&self.gpu, &mixture, FULL_DIP);
            }
            PigmentDip::AddOnTop(slot) => {
                let mut single = self.session.brush.clone();
                single.recipe = vec![(slot, 1.0)];
                let mixture = single.reservoir_mixture(self.slot_count());
                self.session.dip(&self.gpu, &mixture, PARTIAL_DIP);
            }
        }
        self.measure_brush();
        self.wake();
    }

    pub fn set_brush_size(&mut self, diameter_px: f32) {
        self.end_stroke();
        self.session.brush.geometry.max_diameter_px = diameter_px;
        self.session.resize_tip();
    }

    pub fn set_brush_shape(&mut self, shape: BrushShape) {
        self.end_stroke();
        self.session.brush.shape = shape;
        self.session.resize_tip();
    }

    pub fn set_load(&mut self, load: f32) {
        self.session.brush.load = load;
    }

    pub fn set_medium(&mut self, medium: f32) {
        self.session.brush.medium = medium;
    }

    pub fn pick_colour(&mut self, x: f32, y: f32) -> bool {
        self.end_stroke();
        self.session.flush(&self.gpu);
        let before = self.session.engine.reservoir_mixture(&self.gpu);
        let picked = self.session.pick_color(&self.gpu, x, y);
        if picked == before {
            return false;
        }
        self.session.brush.recipe = recipe_from_fractions(&picked.fractions);
        self.session.reload_brush(&self.gpu);
        self.measure_brush();
        self.wake();
        true
    }

    pub fn add_pigment(&mut self, id: &str) -> Result<usize, String> {
        let session = &mut self.session;
        let slot = session.canvas.add_slot(&self.gpu, &session.db, id)?;
        self.edits.record();
        self.wake();
        Ok(slot)
    }

    pub fn dry(&mut self) -> DryOutcome {
        self.end_stroke();
        self.session.flush(&self.gpu);
        if self.session.canvas.wet_tiles.is_empty() {
            return DryOutcome::NothingWet;
        }
        let undoable = self.session.dry(&self.gpu);
        self.edits.record();
        self.wake();
        DryOutcome::Dried { undoable }
    }

    pub fn undo(&mut self) -> bool {
        self.end_stroke();
        let undone = self.session.undo(&self.gpu);
        if undone {
            self.edits.record();
        }
        self.wake();
        undone
    }

    pub fn redo(&mut self) -> bool {
        self.end_stroke();
        let redone = self.session.redo(&self.gpu);
        if redone {
            self.edits.record();
        }
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
        match stroke_preparation(tool, self.reload_each_stroke, self.clean_each_stroke) {
            StrokePreparation::None => {}
            StrokePreparation::RestoreReserve => self.session.restore_reserve(&self.gpu),
            StrokePreparation::CleanAndLoad => self.session.reload_brush(&self.gpu),
        }
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
        self.edits.record();
        self.measure_brush();
        self.wake();
    }

    pub fn encode_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.session.encode_frame(&self.gpu, encoder);
        if !self.stroking {
            self.session.brush.tool = self.resting_tool;
        }
        self.pen.samples_last_frame = std::mem::take(&mut self.samples_since_frame);
    }

    fn measure_brush(&mut self) {
        self.tip_mixture = self.session.tip_mean_mixture(&self.gpu);
        self.reserve_volume = self.session.engine.reserve_totals(&self.gpu).volume as f32;
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
    use oleo_engine::canvas::{PaintTotals, StateTexture};

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
        current.edits.record();

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
        assert!(!current.has_unsaved_edits());
    }

    const WHITE_SLOT: usize = 0;
    const ULTRAMARINE_SLOT: usize = 4;
    const BARE_CANVAS_POINT: (f32, f32) = (20.0, 20.0);
    const RESERVE_TOLERANCE: f64 = 1e-3;

    fn brush_totals(state: &AppState) -> PaintTotals {
        state.session.engine.brush_totals(&state.gpu)
    }

    fn reserve_volume(state: &AppState) -> f64 {
        state.session.engine.reserve_totals(&state.gpu).volume
    }

    fn single_pigment(state: &AppState, slot: usize) -> Mixture {
        let mut brush = state.session.brush.clone();
        brush.recipe = vec![(slot, 1.0)];
        brush.reservoir_mixture(state.slot_count())
    }

    #[test]
    fn resizing_the_brush_keeps_the_paint_on_its_tip() {
        let mut state = painting_state();
        stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
        let before = brush_totals(&state);
        state.set_brush_size(TEST_BRUSH_DIAMETER_PX * 2.0);
        state.set_brush_shape(BrushShape::Flat);
        assert_eq!(brush_totals(&state), before);
    }

    #[test]
    fn load_and_medium_wait_for_the_next_dip() {
        let mut state = painting_state();
        let before = brush_totals(&state);
        state.set_load(0.2);
        state.set_medium(0.5);
        assert_eq!(brush_totals(&state), before);
    }

    #[test]
    fn choosing_another_pigment_cleans_the_brush_before_loading_it() {
        let mut state = painting_state();
        stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
        state.choose_pigment(WHITE_SLOT, PigmentClick::Choose);
        let totals = brush_totals(&state);
        assert_eq!(totals.pigment[ULTRAMARINE_SLOT], 0.0);
        assert!(totals.pigment[WHITE_SLOT] > 0.0);
        assert_eq!(state.session.brush.recipe, [(WHITE_SLOT, 1.0)]);
    }

    #[test]
    fn ctrl_click_mixes_a_little_of_the_pigment_into_what_the_brush_holds() {
        let mut state = painting_state();
        let before = brush_totals(&state);
        state.choose_pigment(WHITE_SLOT, PigmentClick::Mix);
        let after = brush_totals(&state);
        assert!(after.pigment[WHITE_SLOT] > 0.0);
        assert!(after.pigment[WHITE_SLOT] < after.pigment[ULTRAMARINE_SLOT]);
        let kept = after.pigment[ULTRAMARINE_SLOT] / before.pigment[ULTRAMARINE_SLOT];
        assert!((kept - 1.0).abs() < RESERVE_TOLERANCE, "{kept}");
        assert_eq!(
            state.session.brush.recipe,
            [(ULTRAMARINE_SLOT, 1.0), (WHITE_SLOT, PARTIAL_DIP)]
        );
    }

    #[test]
    fn clicking_the_loaded_pigment_again_dips_more_paint_on_top() {
        let mut state = painting_state();
        stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
        let worn = brush_totals(&state);
        state.choose_pigment(ULTRAMARINE_SLOT, PigmentClick::Choose);
        let dipped = brush_totals(&state);
        assert!(dipped.volume > worn.volume);
        assert_eq!(dipped.pigment[WHITE_SLOT], 0.0);
    }

    #[test]
    fn each_stroke_tops_the_reserve_back_up_while_the_tip_keeps_its_dirt() {
        let mut state = painting_state();
        let full = reserve_volume(&state);
        stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
        assert!(reserve_volume(&state) < full);
        let white = single_pigment(&state, WHITE_SLOT);
        state.session.dip(&state.gpu.clone(), &white, PARTIAL_DIP);
        let samples = row_samples(RAG_ROW_Y);
        state.begin_stroke(Tool::Brush, samples[0]);
        assert!((reserve_volume(&state) - full).abs() <= full * RESERVE_TOLERANCE);
        assert!(brush_totals(&state).pigment[WHITE_SLOT] > 0.0);
    }

    #[test]
    fn without_reloading_the_reserve_keeps_running_down() {
        let mut state = painting_state();
        state.reload_each_stroke = false;
        stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
        let worn = reserve_volume(&state);
        let samples = row_samples(RAG_ROW_Y);
        state.begin_stroke(Tool::Brush, samples[0]);
        assert_eq!(reserve_volume(&state), worn);
    }

    #[test]
    fn cleaning_each_stroke_starts_every_stroke_from_the_palette_mix() {
        let mut state = painting_state();
        state.clean_each_stroke = true;
        let white = single_pigment(&state, WHITE_SLOT);
        state.session.dip(&state.gpu.clone(), &white, PARTIAL_DIP);
        assert!(brush_totals(&state).pigment[WHITE_SLOT] > 0.0);
        let samples = row_samples(PAINT_ROW_Y);
        state.begin_stroke(Tool::Brush, samples[0]);
        let totals = brush_totals(&state);
        assert_eq!(totals.pigment[WHITE_SLOT], 0.0);
        assert!(totals.pigment[ULTRAMARINE_SLOT] > 0.0);
    }

    #[test]
    fn eyedropper_on_bare_canvas_changes_nothing() {
        let mut state = painting_state();
        stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
        let before = brush_totals(&state);
        let recipe = state.session.brush.recipe.clone();
        let (x, y) = BARE_CANVAS_POINT;
        assert!(!state.pick_colour(x, y));
        assert_eq!(brush_totals(&state), before);
        assert_eq!(state.session.brush.recipe, recipe);
    }

    #[test]
    fn eyedropper_mixes_the_picked_paint_on_the_palette_and_loads_it() {
        let mut state = painting_state();
        let samples = row_samples(PAINT_ROW_Y);
        stroke(&mut state, Tool::Brush, &samples);
        state.choose_pigment(WHITE_SLOT, PigmentClick::Choose);
        let middle = samples[samples.len() / 2];
        assert!(state.pick_colour(middle.x as f32, middle.y as f32));
        let recipe = &state.session.brush.recipe;
        assert_eq!(recipe.len(), 1, "{recipe:?}");
        assert_eq!(recipe[0].0, ULTRAMARINE_SLOT);
        let totals = brush_totals(&state);
        assert_eq!(totals.pigment[WHITE_SLOT], 0.0);
        assert!(totals.pigment[ULTRAMARINE_SLOT] > 0.0);
    }

    #[test]
    fn drying_a_canvas_without_wet_paint_does_nothing() {
        let mut state = painting_state();
        assert_eq!(state.dry(), DryOutcome::NothingWet);
        assert!(!state.has_unsaved_edits());
        assert!(!state.can_undo());
    }

    #[test]
    fn drying_wet_paint_can_be_undone() {
        let mut state = painting_state();
        stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
        let mark = state.edit_mark();
        state.saved_to(PathBuf::from("seca.oleo"), mark);
        assert_eq!(state.dry(), DryOutcome::Dried { undoable: true });
        assert!(state.has_unsaved_edits());
    }

    #[test]
    fn brush_readout_follows_loading_painting_and_cleaning() {
        let mut state = painting_state();
        assert!(!mixture_is_empty(&state.tip_mixture));
        let loaded = state.reserve_level();
        assert!(loaded > 0.0);
        stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
        assert!(state.reserve_level() < loaded);
        state.clean_brush();
        assert!(state.brush_is_clean());
        assert_eq!(state.reserve_level(), 0.0);
    }

    #[test]
    fn click_on_another_pigment_replaces_the_palette_mix() {
        assert_eq!(
            plan_pigment_dip(&[(0, 2.0), (3, 1.0)], 5, PigmentClick::Choose),
            (vec![(5, 1.0)], PigmentDip::CleanAndLoad)
        );
        assert_eq!(
            plan_pigment_dip(&[(0, 2.0), (3, 1.0)], 3, PigmentClick::Choose),
            (vec![(3, 1.0)], PigmentDip::CleanAndLoad)
        );
    }

    #[test]
    fn click_on_the_only_pigment_dips_again() {
        assert_eq!(
            plan_pigment_dip(&[(2, 1.0)], 2, PigmentClick::Choose),
            (vec![(2, 1.0)], PigmentDip::DipAgain)
        );
    }

    #[test]
    fn shift_click_always_cleans_and_loads() {
        assert_eq!(
            plan_pigment_dip(&[(2, 1.0)], 2, PigmentClick::CleanAndLoad),
            (vec![(2, 1.0)], PigmentDip::CleanAndLoad)
        );
    }

    #[test]
    fn ctrl_click_adds_a_quarter_of_the_pile_of_that_pigment() {
        assert_eq!(
            plan_pigment_dip(&[(0, 1.0)], 3, PigmentClick::Mix),
            (vec![(0, 1.0), (3, 0.25)], PigmentDip::AddOnTop(3))
        );
        assert_eq!(
            recipe_after_mixing(&[(0, 2.0), (3, 2.0)], 3),
            [(0, 2.0), (3, 3.0)]
        );
    }

    #[test]
    fn ctrl_click_on_an_empty_palette_starts_a_pile() {
        assert_eq!(recipe_after_mixing(&[], 2), [(2, 1.0)]);
        assert!(!is_only_pigment(&[], 2));
        assert!(!is_only_pigment(&[(2, 1.0), (3, 0.5)], 2));
        assert!(is_only_pigment(&[(2, 1.0), (3, 0.0)], 2));
    }

    #[test]
    fn only_the_brush_reloads_or_cleans_at_pen_down() {
        assert_eq!(
            stroke_preparation(Tool::Brush, true, false),
            StrokePreparation::RestoreReserve
        );
        assert_eq!(
            stroke_preparation(Tool::Brush, true, true),
            StrokePreparation::CleanAndLoad
        );
        assert_eq!(
            stroke_preparation(Tool::Brush, false, false),
            StrokePreparation::None
        );
        for tool in [Tool::Knife, Tool::Blender, Tool::Rag] {
            assert_eq!(
                stroke_preparation(tool, true, true),
                StrokePreparation::None
            );
        }
    }

    #[test]
    fn reserve_level_is_a_share_of_the_full_reserve() {
        let small = full_reserve_capacity(3.0, 10.0);
        assert!(full_reserve_capacity(3.0, 20.0) > 3.9 * small);
        assert!(full_reserve_capacity(6.0, 10.0) > 1.9 * small);
        assert_eq!(reserve_level(small / 2.0, small), 0.5);
        assert_eq!(reserve_level(small * 2.0, small), 1.0);
        assert_eq!(reserve_level(1.0, 0.0), 0.0);
    }

    #[test]
    fn a_film_without_volume_is_an_empty_brush() {
        let mut fractions = [0.0; MAX_SLOTS];
        fractions[1] = 0.9;
        assert!(mixture_is_empty(&Mixture {
            volume_um: 0.0,
            fractions
        }));
        assert!(mixture_is_empty(&EMPTY_MIXTURE));
        assert!(!mixture_is_empty(&Mixture {
            volume_um: 10.0,
            fractions
        }));
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
