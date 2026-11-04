use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use cadmium_engine::brush::{BrushShape, FULL_RESERVOIR_VOLUME_UM};
use cadmium_engine::canvas::{CanvasError, CanvasSpec, PaintTotals};
use cadmium_engine::document::{DocumentSnapshot, ExportedImage};
use cadmium_engine::engine::{BrushReadout, FULL_DIP, PaintEngine};
use cadmium_engine::gpu::{GpuContext, GpuLost};
use cadmium_engine::params::{ParamGroup, SimParams};
use cadmium_engine::pigments::{MAX_SLOTS, Mixture};
use cadmium_engine::present::ViewTransform;
use cadmium_engine::render::ViewMode;
use cadmium_engine::session::{ClearOutcome, PaintSession};
use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;

use crate::presets::{STARTING_LOAD, starting_recipe};
use crate::tools::UiTool;
use crate::writes::{EditCounter, EditMark};
use gesture::Contact;
pub use gesture::LoadRing;

mod gesture;

const RATE_SMOOTHING: f32 = 0.2;
const MILLISECONDS_PER_SECOND: f32 = 1000.0;
const NEGLIGIBLE_PICKED_FRACTION: f32 = 1e-3;
pub const PARTIAL_DIP: f32 = 0.25;
const KNIFE_TOP_UP_TOLERANCE: f32 = 1e-3;

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

pub fn knife_top_up(volume: f32, target: f32) -> Option<f32> {
    if !target.is_finite() || target <= 0.0 || !volume.is_finite() {
        return None;
    }
    let missing = target - volume.max(0.0);
    (missing > target * KNIFE_TOP_UP_TOLERANCE).then(|| (missing / target).min(FULL_DIP))
}

pub fn mean_mixture(totals: &PaintTotals) -> Mixture {
    if !totals.volume.is_finite() || totals.volume <= 0.0 {
        return EMPTY_MIXTURE;
    }
    Mixture {
        volume_um: totals.volume as f32,
        fractions: std::array::from_fn(|slot| (totals.pigment[slot] / totals.volume) as f32),
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
    TopUp,
    CleanAndLoad,
}

pub fn stroke_preparation(
    tool: Tool,
    reload_each_stroke: bool,
    clean_each_stroke: bool,
) -> StrokePreparation {
    match tool {
        Tool::Brush | Tool::Knife if clean_each_stroke => StrokePreparation::CleanAndLoad,
        Tool::Brush | Tool::Knife if reload_each_stroke => StrokePreparation::TopUp,
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
    pub tube_slot: usize,
    edits: EditCounter,
    pub file: Option<PathBuf>,
    pub pen: PenReadout,
    pub tip_mixture: Mixture,
    pub reload_each_stroke: bool,
    pub clean_each_stroke: bool,
    reserve_volume: f32,
    pub knife_mixture: Mixture,
    knife_volume: f32,
    contact: Option<Contact>,
    hold_clock: Option<Instant>,
    tray_zoom: f32,
    resting_tool: Tool,
    samples_since_frame: u32,
    last_sample_time_ms: Option<u32>,
    wakers: Vec<Waker>,
}

impl AppState {
    pub fn new(gpu: Rc<GpuContext>, spec: CanvasSpec) -> Result<AppState, CanvasError> {
        let mut session = PaintSession::new(&gpu, spec)?;
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
            tube_slot: 0,
            edits: EditCounter::default(),
            file: None,
            pen: PenReadout::default(),
            tip_mixture: EMPTY_MIXTURE,
            reload_each_stroke: true,
            clean_each_stroke: false,
            reserve_volume: 0.0,
            knife_mixture: EMPTY_MIXTURE,
            knife_volume: 0.0,
            contact: None,
            hold_clock: None,
            tray_zoom: 1.0,
            resting_tool,
            samples_since_frame: 0,
            last_sample_time_ms: None,
            wakers: Vec::new(),
        };
        state.reload_brush();
        Ok(state)
    }

    pub fn add_waker(&mut self, waker: Waker) {
        self.wakers.push(waker);
    }

    fn wake(&self) {
        for waker in &self.wakers {
            waker();
        }
    }

    pub fn start_painting(&mut self, spec: CanvasSpec) -> Result<(), CanvasError> {
        self.release();
        let old_palette = self.session.canvas.spec.pigment_slots.clone();
        let replaced = self.session.replace_document(&self.gpu, spec);
        self.adopt_document(&old_palette, replaced.map(|()| None));
        replaced
    }

    pub fn open_painting(
        &mut self,
        snapshot: DocumentSnapshot,
        path: PathBuf,
    ) -> Result<(), CanvasError> {
        self.release();
        let old_palette = self.session.canvas.spec.pigment_slots.clone();
        let restored = snapshot.restore_into(&self.gpu, &mut self.session);
        self.adopt_document(&old_palette, restored.map(|()| Some(path)));
        restored
    }

    pub fn capture_document(&mut self) -> Result<DocumentSnapshot, GpuLost> {
        DocumentSnapshot::capture(&self.gpu, &mut self.session)
    }

    pub fn render_export(&mut self) -> Result<ExportedImage, GpuLost> {
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

    fn adopt_document(
        &mut self,
        old_palette: &[String],
        file: Result<Option<PathBuf>, CanvasError>,
    ) {
        let palette = &self.session.canvas.spec.pigment_slots;
        let brush = &mut self.session.brush;
        brush.tool = self.resting_tool;
        brush.recipe = recipe_for_palette(&brush.recipe, old_palette, palette);
        if brush.recipe.is_empty() {
            brush.recipe = starting_recipe(palette);
        }
        if self.tube_slot >= palette.len() {
            self.tube_slot = 0;
        }
        self.reload_brush();
        match file {
            Ok(file) => {
                self.edits.forget_unsaved();
                self.file = file;
            }
            Err(_) => self.file = None,
        }
        self.wake();
    }

    pub fn slot_count(&self) -> usize {
        self.session.canvas.spec.pigment_slots.len()
    }

    pub fn palette_mixture(&self) -> Mixture {
        self.session.brush.reservoir_mixture(self.slot_count())
    }

    pub fn reserve_level(&self) -> f32 {
        reserve_level(
            self.reserve_volume,
            self.session.engine.full_reserve_volume(),
        )
    }

    pub fn brush_is_clean(&self) -> bool {
        self.tip_mixture.is_empty() && self.reserve_volume <= 0.0
    }

    pub fn knife_level(&self) -> f32 {
        let capacity = self
            .session
            .engine
            .knife_full_dip_volume(FULL_RESERVOIR_VOLUME_UM);
        reserve_level(self.knife_volume, capacity)
    }

    pub fn knife_is_clean(&self) -> bool {
        self.knife_volume <= 0.0
    }

    pub fn holds_knife(&self) -> bool {
        self.session.brush.tool == Tool::Knife
    }

    fn put_down_stroke_tool(&mut self) {
        self.release();
        self.session.brush.tool = self.resting_tool;
    }

    fn clean_and_load(&mut self) {
        if !self.holds_knife() {
            self.session.reload_brush(&self.gpu);
            return;
        }
        self.session.clean_brush(&self.gpu);
        let mixture = self.palette_mixture();
        if !mixture.is_empty() {
            self.dip(&mixture, FULL_DIP);
        }
    }

    fn dip(&mut self, mixture: &Mixture, amount: f32) {
        if self.session.dip(&self.gpu, mixture, amount).is_err() {
            self.wake();
        }
    }

    fn top_up(&mut self) {
        if !self.holds_knife() {
            self.session.restore_reserve(&self.gpu);
            return;
        }
        let mixture = self.palette_mixture();
        if mixture.is_empty() {
            return;
        }
        self.session.flush(&self.gpu);
        self.session.resize_tip();
        let Ok(totals) = self.session.engine.knife_totals(&self.gpu) else {
            self.wake();
            return;
        };
        let target = self.session.engine.knife_full_dip_volume(mixture.volume_um);
        if let Some(amount) = knife_top_up(totals.volume as f32, target) {
            self.dip(&mixture, amount);
        }
    }

    pub fn reload_brush(&mut self) {
        self.release();
        self.session.reload_brush(&self.gpu);
        self.measure_brush();
    }

    pub fn clean_brush(&mut self) {
        self.put_down_stroke_tool();
        self.session.clean_brush(&self.gpu);
        self.measure_brush();
        self.wake();
    }

    pub fn choose_pigment(&mut self, slot: usize, click: PigmentClick) {
        self.put_down_stroke_tool();
        let (recipe, dip) = plan_pigment_dip(&self.session.brush.recipe, slot, click);
        self.session.brush.recipe = recipe;
        match dip {
            PigmentDip::CleanAndLoad => self.clean_and_load(),
            PigmentDip::DipAgain => {
                let mixture = self.palette_mixture();
                self.dip(&mixture, FULL_DIP);
            }
            PigmentDip::AddOnTop(slot) => {
                let mut single = self.session.brush.clone();
                single.recipe = vec![(slot, 1.0)];
                let mixture = single.reservoir_mixture(self.slot_count());
                self.dip(&mixture, PARTIAL_DIP);
            }
        }
        self.measure_brush();
        self.wake();
    }

    pub fn set_brush_size(&mut self, diameter_px: f32) {
        self.release();
        self.session.brush.geometry.max_diameter_px = diameter_px;
        self.session.resize_tip();
    }

    pub fn set_brush_shape(&mut self, shape: BrushShape) {
        self.release();
        self.session.brush.shape = shape;
        self.session.resize_tip();
    }

    pub fn set_load(&mut self, load: f32) {
        self.session.brush.load = load;
    }

    pub fn set_medium(&mut self, medium: f32) {
        self.session.brush.medium = medium;
    }

    pub fn pick_colour_on(&mut self, surface: Surface, x: f32, y: f32) -> bool {
        self.put_down_stroke_tool();
        let Ok(Some(picked)) = self.session.pick_colour_on(&self.gpu, surface, x, y) else {
            self.wake();
            return false;
        };
        self.session.brush.recipe = recipe_from_fractions(&picked.fractions);
        self.session.reload_brush(&self.gpu);
        if self.holds_knife() {
            self.clean_and_load();
        }
        self.measure_brush();
        self.wake();
        true
    }

    pub fn add_pigment(&mut self, id: &str) -> Result<usize, String> {
        let slot = self.session.add_slot(&self.gpu, id)?;
        self.edits.record();
        self.wake();
        Ok(slot)
    }

    pub fn dry(&mut self) -> DryOutcome {
        self.release();
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
        self.release();
        let undone = self.session.undo(&self.gpu);
        if undone {
            self.edits.record();
        }
        self.wake();
        undone
    }

    pub fn redo(&mut self) -> bool {
        self.release();
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

    pub fn history_is_enabled(&self) -> bool {
        self.session.history.is_enabled()
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
        if !self.is_stroking() {
            self.session.brush.tool = paint_tool;
        }
    }

    pub fn set_tube_slot(&mut self, slot: usize) {
        if slot < self.slot_count() {
            self.tube_slot = slot;
        }
    }

    pub fn clear_tray(&mut self) {
        self.release();
        if let ClearOutcome::Cleared { .. } = self.session.clear_tray(&self.gpu) {
            self.edits.record();
        }
        self.wake();
    }

    pub fn observe_hover_pen(&mut self, sample: &StrokeSample) {
        self.pen.observe_hover(sample);
    }

    pub fn encode_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.advance_hold_by_clock(Surface::Painting);
        self.session.encode_frame(&self.gpu, encoder);
        if !self.is_stroking() {
            self.session.brush.tool = self.resting_tool;
        }
        self.pen.samples_last_frame = std::mem::take(&mut self.samples_since_frame);
    }

    pub fn encode_tray_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.advance_hold_by_clock(Surface::Tray);
        self.session.encode_tray_frame(&self.gpu, encoder);
    }

    fn measure_brush(&mut self) -> Option<BrushReadout> {
        let Ok(readout) = self.session.read_brush(&self.gpu) else {
            self.wake();
            return None;
        };
        self.tip_mixture = readout.tip_mean;
        self.reserve_volume = readout.reserve.volume as f32;
        self.knife_mixture = mean_mixture(&readout.knife);
        self.knife_volume = readout.knife.volume as f32;
        Some(readout)
    }

    pub fn gpu_is_lost(&self) -> bool {
        self.gpu.is_lost()
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
    use cadmium_engine::canvas::StateTexture;

    use super::*;
    use crate::presets::starting_canvas_spec;

    const TEST_CANVAS: (u32, u32) = (512, 256);
    const TEST_BRUSH_DIAMETER_PX: f32 = 16.0;
    const PAINT_ROW_Y: f64 = 60.0;
    const RAG_ROW_Y: f64 = 200.0;
    const STROKE_STEPS: u32 = 20;

    pub(super) fn painting_state() -> AppState {
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
        state.release();
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
        let snapshot = saved.capture_document().unwrap();
        let path = PathBuf::from("opened.cadmium");
        current.open_painting(snapshot, path.clone()).unwrap();

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

    #[test]
    fn a_new_painting_keeps_the_lab_and_starts_untitled_without_undo() {
        const TUNED_PARAM_VALUE_SHARE: f32 = 0.75;
        let mut state = painting_state();
        let spec = &SimParams::specs()[0];
        let tuned = spec.min + (spec.max - spec.min) * TUNED_PARAM_VALUE_SHARE;
        state.set_param(spec.key, tuned);
        stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
        let mark = state.edit_mark();
        state.saved_to(PathBuf::from("antiga.cadmium"), mark);
        let blank = state.session.canvas.spec.clone();
        state.start_painting(blank).unwrap();
        assert_eq!(state.file, None);
        assert!(!state.can_undo());
        assert!(!state.has_unsaved_edits());
        assert_eq!(wet_volume(&state), 0.0);
        assert_eq!(state.session.engine.params().get(spec.key), tuned);
        assert!(!state.brush_is_clean());
    }

    pub(super) const WHITE_SLOT: usize = 0;
    pub(super) const ULTRAMARINE_SLOT: usize = 4;
    const BARE_CANVAS_POINT: (f32, f32) = (20.0, 20.0);
    const RESERVE_TOLERANCE: f64 = 1e-3;
    const DIRT_DIP: f32 = 0.1;

    pub(super) fn brush_totals(state: &AppState) -> PaintTotals {
        state.session.engine.brush_totals(&state.gpu).unwrap()
    }

    fn reserve_volume(state: &AppState) -> f64 {
        state
            .session
            .engine
            .reserve_totals(&state.gpu)
            .unwrap()
            .volume
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
        state
            .session
            .dip(&state.gpu.clone(), &white, DIRT_DIP)
            .unwrap();
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
        state
            .session
            .dip(&state.gpu.clone(), &white, PARTIAL_DIP)
            .unwrap();
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
        assert!(!state.pick_colour_on(Surface::Painting, x, y));
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
        assert!(state.pick_colour_on(Surface::Painting, middle.x as f32, middle.y as f32));
        let recipe = &state.session.brush.recipe;
        assert_eq!(recipe.len(), 1, "{recipe:?}");
        assert_eq!(recipe[0].0, ULTRAMARINE_SLOT);
        let totals = brush_totals(&state);
        assert_eq!(totals.pigment[WHITE_SLOT], 0.0);
        assert!(totals.pigment[ULTRAMARINE_SLOT] > 0.0);
    }

    #[test]
    fn picking_the_colour_already_on_the_brush_refreshes_it() {
        let mut state = painting_state();
        let full = reserve_volume(&state);
        let samples = row_samples(PAINT_ROW_Y);
        state.reload_each_stroke = false;
        stroke(&mut state, Tool::Brush, &samples);
        assert!(reserve_volume(&state) < full);
        let middle = samples[samples.len() / 2];
        assert!(state.pick_colour_on(Surface::Painting, middle.x as f32, middle.y as f32));
        assert!((reserve_volume(&state) - full).abs() <= full * RESERVE_TOLERANCE);
        assert_eq!(state.session.brush.recipe.len(), 1);
        assert_eq!(state.session.brush.recipe[0].0, ULTRAMARINE_SLOT);
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
        state.saved_to(PathBuf::from("seca.cadmium"), mark);
        assert_eq!(state.dry(), DryOutcome::Dried { undoable: true });
        assert!(state.has_unsaved_edits());
    }

    #[test]
    fn brush_readout_follows_loading_painting_and_cleaning() {
        let mut state = painting_state();
        assert!(!state.tip_mixture.is_empty());
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
    fn brush_and_knife_reload_or_clean_at_pen_down_and_other_tools_do_not() {
        for tool in [Tool::Brush, Tool::Knife] {
            assert_eq!(
                stroke_preparation(tool, true, false),
                StrokePreparation::TopUp
            );
            assert_eq!(
                stroke_preparation(tool, true, true),
                StrokePreparation::CleanAndLoad
            );
            assert_eq!(
                stroke_preparation(tool, false, false),
                StrokePreparation::None
            );
        }
        for tool in [Tool::Blender, Tool::Rag, Tool::Eyedropper] {
            assert_eq!(
                stroke_preparation(tool, true, true),
                StrokePreparation::None
            );
        }
    }

    #[test]
    fn knife_tops_up_only_the_share_it_is_missing() {
        assert_eq!(knife_top_up(0.0, 400.0), Some(1.0));
        assert_eq!(knife_top_up(300.0, 400.0), Some(0.25));
        assert_eq!(knife_top_up(400.0, 400.0), None);
        assert_eq!(knife_top_up(500.0, 400.0), None);
        assert_eq!(knife_top_up(-5.0, 400.0), Some(1.0));
        assert_eq!(knife_top_up(0.0, 0.0), None);
        assert_eq!(knife_top_up(f32::NAN, 400.0), None);
    }

    #[test]
    fn knife_colour_is_the_mean_of_what_it_carries() {
        let mut pigment = [0.0; MAX_SLOTS];
        pigment[1] = 30.0;
        pigment[3] = 60.0;
        let mixture = mean_mixture(&PaintTotals {
            volume: 100.0,
            pigment,
        });
        assert_eq!(mixture.volume_um, 100.0);
        assert!((mixture.fractions[1] - 0.3).abs() < 1e-6);
        assert!((mixture.fractions[3] - 0.6).abs() < 1e-6);
        assert!(mean_mixture(&PaintTotals::default()).is_empty());
    }

    const KNIFE_LEVEL_TOLERANCE: f32 = 0.01;

    fn knife_totals(state: &AppState) -> PaintTotals {
        state.session.engine.knife_totals(&state.gpu).unwrap()
    }

    fn holding_the_knife() -> AppState {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Paint(Tool::Knife));
        state
    }

    #[test]
    fn a_swatch_click_with_the_knife_loads_the_knife_and_leaves_the_brush_alone() {
        let mut state = holding_the_knife();
        let brush = brush_totals(&state);
        state.choose_pigment(WHITE_SLOT, PigmentClick::Choose);
        let knife = knife_totals(&state);
        assert!(knife.pigment[WHITE_SLOT] > 0.0);
        assert_eq!(knife.pigment[ULTRAMARINE_SLOT], 0.0);
        assert_eq!(brush_totals(&state), brush);
        let level = state.knife_level();
        assert!(
            (level - state.session.brush.load).abs() < KNIFE_LEVEL_TOLERANCE,
            "{level}"
        );
        assert!(!state.knife_is_clean());
        assert!(state.knife_mixture.fractions[WHITE_SLOT] > 0.0);
    }

    #[test]
    fn another_pigment_on_the_knife_cleans_it_first_and_ctrl_click_adds_a_little() {
        let mut state = holding_the_knife();
        state.choose_pigment(WHITE_SLOT, PigmentClick::Choose);
        state.choose_pigment(ULTRAMARINE_SLOT, PigmentClick::Choose);
        let full = knife_totals(&state);
        assert_eq!(full.pigment[WHITE_SLOT], 0.0);
        state.choose_pigment(WHITE_SLOT, PigmentClick::Mix);
        let mixed = knife_totals(&state);
        assert!(mixed.pigment[WHITE_SLOT] > 0.0);
        assert!(mixed.pigment[WHITE_SLOT] < mixed.pigment[ULTRAMARINE_SLOT]);
        assert_eq!(
            state.session.brush.recipe,
            [(ULTRAMARINE_SLOT, 1.0), (WHITE_SLOT, PARTIAL_DIP)]
        );
    }

    #[test]
    fn cleaning_the_knife_keeps_the_palette_pile_and_the_brush() {
        let mut state = holding_the_knife();
        state.choose_pigment(ULTRAMARINE_SLOT, PigmentClick::Choose);
        let brush = brush_totals(&state);
        let recipe = state.session.brush.recipe.clone();
        state.clean_brush();
        assert_eq!(knife_totals(&state).volume, 0.0);
        assert!(state.knife_is_clean());
        assert_eq!(state.knife_level(), 0.0);
        assert_eq!(state.session.brush.recipe, recipe);
        assert_eq!(brush_totals(&state), brush);
    }

    #[test]
    fn knife_comes_down_loaded_to_the_palette_load_and_no_further() {
        let mut state = holding_the_knife();
        assert_eq!(knife_totals(&state).volume, 0.0);
        let samples = row_samples(PAINT_ROW_Y);
        state.begin_stroke(Tool::Knife, samples[0]);
        let mixture = state.palette_mixture();
        let target = f64::from(
            state
                .session
                .engine
                .knife_full_dip_volume(mixture.volume_um),
        );
        let loaded = knife_totals(&state).volume;
        assert!(
            (loaded - target).abs() <= target * RESERVE_TOLERANCE,
            "{loaded} {target}"
        );
        state.release();
        state.begin_stroke(Tool::Knife, samples[0]);
        let again = knife_totals(&state).volume;
        assert!(
            again <= target * (1.0 + RESERVE_TOLERANCE),
            "{again} {target}"
        );
        state.release();
    }

    #[test]
    fn without_reloading_the_knife_stays_as_empty_as_it_is() {
        let mut state = holding_the_knife();
        state.reload_each_stroke = false;
        let samples = row_samples(PAINT_ROW_Y);
        state.begin_stroke(Tool::Knife, samples[0]);
        assert_eq!(knife_totals(&state).volume, 0.0);
        state.release();
    }

    #[test]
    fn reserve_level_is_a_share_of_the_full_reserve() {
        let small = 100.0;
        assert_eq!(reserve_level(small / 2.0, small), 0.5);
        assert_eq!(reserve_level(small * 2.0, small), 1.0);
        assert_eq!(reserve_level(1.0, 0.0), 0.0);
    }

    fn sample_timed(time_ms: u32) -> StrokeSample {
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
        readout.record(&sample_timed(0), None);
        readout.record(&sample_timed(5), Some(0));
        assert!((readout.rate_hz - 200.0).abs() < 1e-3);
        assert_eq!(readout.tilt_y, -20.0);
    }

    #[test]
    fn samples_with_the_same_timestamp_leave_the_rate_alone() {
        let mut readout = PenReadout {
            rate_hz: 120.0,
            ..PenReadout::default()
        };
        readout.record(&sample_timed(7), Some(7));
        assert_eq!(readout.rate_hz, 120.0);
    }

    #[test]
    fn clock_going_backwards_does_not_spike_the_rate() {
        let mut readout = PenReadout::default();
        readout.record(&sample_timed(3), Some(10));
        assert_eq!(readout.rate_hz, 0.0);
    }
}
