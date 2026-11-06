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

use crate::brush_library::BrushLibrary;
use crate::presets::{STARTING_LOAD, starting_recipe};
use crate::tools::UiTool;
use crate::writes::EditCounter;
use clock::{Clock, SystemClock};
use gesture::Contact;
pub use gesture::LoadRing;
pub use history::DryOutcome;
pub use pen::PenReadout;
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
mod pen;
mod recipe;
mod tray;

const EMPTY_MIXTURE: Mixture = Mixture {
    volume_um: 0.0,
    fractions: [0.0; MAX_SLOTS],
};

pub type SharedState = Rc<RefCell<AppState>>;
pub type Waker = Rc<dyn Fn()>;

pub struct AppState {
    pub gpu: Rc<GpuContext>,
    pub session: PaintSession,
    pub brushes: BrushLibrary,
    /// The library brush last picked, told apart from others with the same head and hair.
    worn_brush_index: Option<usize>,
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
    clock: Box<dyn Clock>,
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
            brushes: BrushLibrary::factory_only(),
            worn_brush_index: None,
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

    fn wake(&self) {
        for waker in &self.wakers {
            waker();
        }
    }
}

#[cfg(test)]
mod test_support;
