use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use cadmium_engine::canvas::{CanvasError, CanvasSpec};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::pigments::{MAX_SLOTS, Mixture};
use cadmium_engine::present::ViewTransform;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;

use crate::brush_library::BrushLibrary;
use crate::presets::{STARTING_LOAD, starting_recipe};
use crate::settings::SettingsStore;
use crate::tools::{TrayTool, UiTool};
use crate::writes::EditCounter;
use clock::{Clock, SystemClock};
use gesture::Contact;
pub use gesture::LoadRing;
pub use history::DryOutcome;
use pacing::FramePacing;
pub use pen::PenReadout;
use provisional::Provisional;
pub use provisional::{Mark, ProvisionalTrail};
pub use recipe::PigmentClick;

mod clock;
mod dip;
mod document;
mod frames;
mod gesture;
mod hand;
mod history;
mod lab;
mod library;
mod mixing_knife;
mod pacing;
mod pen;
mod provisional;
mod recipe;
mod studio_paint;
mod surface_tools;
mod tray;

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
    pub session: PaintSession,
    pub brushes: BrushLibrary,
    pub settings: SettingsStore,
    /// The library brush last picked, told apart from others with the same head and hair.
    worn_brush_index: Option<usize>,
    pub view: ViewTransform,
    pub ui_tool: UiTool,
    tray_tool: TrayTool,
    /// The surface whose tool was chosen last; the side panel describes that tool.
    focus: Surface,
    pub tube_slot: usize,
    edits: EditCounter,
    pub file: Option<PathBuf>,
    pub pen: PenReadout,
    pub tip_mixture: Mixture,
    /// What the tray blender's own tip holds, as last read.
    pub tray_tip_mixture: Mixture,
    pub reload_each_stroke: bool,
    pub clean_each_stroke: bool,
    reserve_volume: f32,
    reserve_mixture: Mixture,
    pub knife_mixture: Mixture,
    knife_volume: f32,
    mixing_knife: mixing_knife::MixingKnife,
    contact: Option<Contact>,
    hold_clock: Option<Instant>,
    /// How the tray widget shows the tray, in device px.
    pub tray_view: ViewTransform,
    /// Device px per widget px on the tray's screen.
    tray_device_scale: f64,
    resting_tool: Tool,
    samples_since_frame: u32,
    pacing: FramePacing,
    provisional: Provisional,
    last_sample_time_ms: Option<u32>,
    clock: Box<dyn Clock>,
    wakers: Vec<Waker>,
}

impl AppState {
    pub fn new(gpu: Rc<GpuContext>, spec: CanvasSpec) -> Result<AppState, CanvasError> {
        let session = starting_session(&gpu, spec)?;
        let resting_tool = session.brush.tool;
        let mut state = AppState {
            gpu,
            session,
            brushes: BrushLibrary::factory_only(),
            settings: SettingsStore::default(),
            worn_brush_index: None,
            view: UNMOVED_VIEW,
            ui_tool: UiTool::default(),
            tray_tool: TrayTool::default(),
            focus: Surface::Painting,
            tube_slot: 0,
            edits: EditCounter::default(),
            file: None,
            pen: PenReadout::default(),
            tip_mixture: EMPTY_MIXTURE,
            tray_tip_mixture: EMPTY_MIXTURE,
            reload_each_stroke: true,
            clean_each_stroke: false,
            reserve_volume: 0.0,
            reserve_mixture: EMPTY_MIXTURE,
            knife_mixture: EMPTY_MIXTURE,
            knife_volume: 0.0,
            mixing_knife: mixing_knife::MixingKnife::default(),
            contact: None,
            hold_clock: None,
            tray_view: UNMOVED_VIEW,
            tray_device_scale: 1.0,
            resting_tool,
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
