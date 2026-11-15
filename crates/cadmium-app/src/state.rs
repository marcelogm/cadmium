use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use cadmium_engine::canvas::{CanvasError, CanvasSpec};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::pigments::{MAX_SLOTS, Mixture};
use cadmium_engine::present::ViewTransform;
use cadmium_engine::session::PaintSession;

use crate::brush_library::BrushLibrary;
use crate::presets::{STARTING_LOAD, starting_recipe};
use crate::settings::SettingsStore;
use crate::writes::EditCounter;
use clock::{Clock, SystemClock};
pub use easel::{Easel, EaselId};
pub use history::DryOutcome;
use pacing::FramePacing;
pub use palette::PaletteView;
pub use palette_edit::{PaletteEdit, PaletteMode};
pub use pen::PenReadout;
use provisional::Provisional;
pub use provisional::{Mark, ProvisionalTrail};
pub use recipe::ColourClick;

mod clock;
mod dip;
mod document;
mod easel;
mod frames;
mod gesture;
mod hand;
mod history;
mod lab;
mod library;
mod pacing;
mod palette;
mod palette_edit;
mod pen;
mod provisional;
mod recipe;
mod studio_paint;

const EMPTY_MIXTURE: Mixture = Mixture {
    volume_um: 0.0,
    fractions: [0.0; MAX_SLOTS],
};

const UNMOVED_VIEW: ViewTransform = ViewTransform {
    origin_x: 0.0,
    origin_y: 0.0,
    zoom: 1.0,
    rotation: 0.0,
};

pub type SharedState = Rc<RefCell<AppState>>;

/// Whether a stroke is running on `state`; a borrowed or missing state counts as one, so
/// nothing that waits for the stroke goes ahead.
pub fn stroking_or_busy(state: Option<&SharedState>) -> bool {
    state
        .and_then(|state| state.try_borrow().ok().map(|state| state.is_stroking()))
        .unwrap_or(true)
}

fn starting_session(gpu: &GpuContext, spec: CanvasSpec) -> Result<PaintSession, CanvasError> {
    let mut session = PaintSession::new(gpu, spec)?;
    session.brush.recipe = starting_recipe(&session.canvas.spec.pigment_slots);
    session.brush.load = STARTING_LOAD;
    Ok(session)
}
/// Wakes one view; returns false once the view is gone, which drops the waker.
///
/// A waker runs under the caller's `borrow_mut` of the state, so it must not
/// borrow `AppState` or its `SharedState`: it only schedules work
/// (`request_frame`, `queue_draw`) for the view to do later.
pub type Waker = Rc<dyn Fn() -> bool>;

pub struct AppState {
    pub gpu: Rc<GpuContext>,
    pub painting: Easel,
    pub sheet: Easel,
    pub palette_edit: PaletteEdit,
    editing_palette: bool,
    pub brushes: BrushLibrary,
    pub settings: SettingsStore,
    /// The library brush last picked, told apart from others with the same head and hair.
    worn_brush_index: Option<usize>,
    pub view: ViewTransform,
    pub palette_view: PaletteView,
    circle_darken: f32,
    edits: EditCounter,
    pub file: Option<PathBuf>,
    pub pen: PenReadout,
    samples_since_frame: u32,
    pacing: FramePacing,
    provisional: Provisional,
    last_sample_time_ms: Option<u32>,
    clock: Box<dyn Clock>,
    wakers: Vec<Waker>,
}

impl AppState {
    pub fn new(gpu: Rc<GpuContext>, spec: CanvasSpec) -> Result<AppState, CanvasError> {
        let painting = Easel::new(starting_session(&gpu, spec)?);
        let slots = &painting.session.canvas.spec.pigment_slots;
        let sheet = Easel::new(PaintSession::for_sheet(&gpu, slots)?);
        let mut state = AppState {
            gpu,
            painting,
            sheet,
            palette_edit: PaletteEdit::default(),
            editing_palette: false,
            brushes: BrushLibrary::factory_only(),
            settings: SettingsStore::default(),
            worn_brush_index: None,
            view: UNMOVED_VIEW,
            palette_view: PaletteView::default(),
            circle_darken: 0.0,
            edits: EditCounter::default(),
            file: None,
            pen: PenReadout::default(),
            samples_since_frame: 0,
            pacing: FramePacing::default(),
            provisional: Provisional::default(),
            last_sample_time_ms: None,
            clock: Box::new(SystemClock::new()),
            wakers: Vec::new(),
        };
        state.reload_brush();
        Ok(state)
    }

    pub fn now_ms(&self) -> f64 {
        self.clock.now_ms()
    }

    #[cfg(test)]
    fn use_clock(&mut self, clock: Box<dyn Clock>) {
        self.clock = clock;
    }

    pub fn easel(&self, id: EaselId) -> &Easel {
        match id {
            EaselId::Painting => &self.painting,
            EaselId::Sheet => &self.sheet,
        }
    }

    pub fn easel_mut(&mut self, id: EaselId) -> &mut Easel {
        match id {
            EaselId::Painting => &mut self.painting,
            EaselId::Sheet => &mut self.sheet,
        }
    }

    pub fn add_waker(&mut self, waker: Waker) {
        self.wakers.push(waker);
    }

    /// Wakes every view, dropping the wakers of views that are gone.
    fn wake(&mut self) {
        self.wakers.retain(|waker| waker());
    }
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
