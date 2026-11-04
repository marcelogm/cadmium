use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use cadmium_engine::brush::FULL_RESERVOIR_VOLUME_UM;
use cadmium_engine::canvas::{CanvasError, CanvasSpec, PaintTotals};
use cadmium_engine::engine::{BrushReadout, FULL_DIP};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::pigments::{MAX_SLOTS, Mixture};
use cadmium_engine::present::ViewTransform;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;

use crate::presets::{STARTING_LOAD, starting_recipe};
use crate::tools::UiTool;
use crate::writes::EditCounter;
use gesture::Contact;
pub use gesture::LoadRing;
pub use history::DryOutcome;
pub use pen::PenReadout;

mod document;
mod frames;
mod gesture;
mod hand;
mod history;
mod lab;
mod pen;
mod tray;

const NEGLIGIBLE_PICKED_FRACTION: f32 = 1e-3;
pub const PARTIAL_DIP: f32 = 0.25;
const KNIFE_TOP_UP_TOLERANCE: f32 = 1e-3;

const EMPTY_MIXTURE: Mixture = Mixture {
    volume_um: 0.0,
    fractions: [0.0; MAX_SLOTS],
};

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
    reserve_mixture: Mixture,
    pub knife_mixture: Mixture,
    knife_volume: f32,
    contact: Option<Contact>,
    hold_clock: Option<Instant>,
    tray_zoom: f32,
    painting_device_scale: f32,
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
            reserve_mixture: EMPTY_MIXTURE,
            knife_mixture: EMPTY_MIXTURE,
            knife_volume: 0.0,
            contact: None,
            hold_clock: None,
            tray_zoom: 1.0,
            painting_device_scale: 1.0,
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

    fn measure_brush(&mut self) -> Option<BrushReadout> {
        let Ok(readout) = self.session.read_brush(&self.gpu) else {
            self.wake();
            return None;
        };
        self.tip_mixture = readout.tip_mean;
        self.reserve_volume = readout.reserve.volume as f32;
        self.reserve_mixture = mean_mixture(&readout.reserve);
        self.knife_mixture = mean_mixture(&readout.knife);
        self.knife_volume = readout.knife.volume as f32;
        Some(readout)
    }
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::{
        PAINT_ROW_Y, RESERVE_TOLERANCE, ULTRAMARINE_SLOT, WHITE_SLOT, brush_totals,
        holding_the_knife, knife_totals, painting_state, reserve_volume, row_samples, stroke,
    };

    #[test]
    fn recipe_follows_its_pigments_into_the_new_palette() {
        let old: Vec<String> = ["white", "blue", "red"].map(String::from).to_vec();
        let new: Vec<String> = ["red", "white"].map(String::from).to_vec();
        assert_eq!(
            recipe_for_palette(&[(0, 0.5), (1, 0.2), (2, 0.3)], &old, &new),
            [(1, 0.5), (0, 0.3)]
        );
    }

    const BARE_CANVAS_POINT: (f32, f32) = (20.0, 20.0);

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
    fn reserve_level_is_a_share_of_the_full_reserve() {
        let small = 100.0;
        assert_eq!(reserve_level(small / 2.0, small), 0.5);
        assert_eq!(reserve_level(small * 2.0, small), 1.0);
        assert_eq!(reserve_level(1.0, 0.0), 0.0);
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
}
