use crate::brush::BrushSettings;
use crate::canvas::{Canvas, CanvasError, CanvasSpec, TILE_SIZE};
use crate::draw_up::DrawUp;
use crate::engine::{PaintEngine, PickRequest, WetOilEngine};
use crate::gpu::{GpuContext, GpuLost, new_encoder};
use crate::history::{History, bytes_per_tile_all_textures};
use crate::pigments::{Mixture, PigmentDb};
use crate::present::Presenter;
use crate::render::{Renderer, ShadeRequest, ViewMode};
use crate::stroke::{Dab, StrokeBuilder};
use crate::surface::{Surface, TRAY_HEIGHT, TRAY_WIDTH, tray_spec};
use crate::tube::Tube;

mod brush;
mod hold;
mod removal;
mod replacement;
mod settle;
mod stroke;

use hold::Hold;
use settle::MotionAnchor;

const PICK_RADIUS_PX: f32 = 4.0;
const HISTORY_BUDGET_BYTES: u64 = 512 * 1024 * 1024;
const HISTORY_MAX_STEPS: usize = 100;
const HISTORY_PAINTING_COPIES: u64 = 4;
const HISTORY_TRAY_COPIES: u64 = 1;

struct UndoLimits {
    budget_bytes: u64,
    max_steps: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearOutcome {
    NothingWet,
    Cleared { undoable: bool },
}

pub struct PaintSession {
    pub canvas: Canvas,
    pub tray: Canvas,
    pub engine: WetOilEngine,
    pub renderer: Renderer,
    pub presenter: Presenter,
    pub history: History,
    pub brush: BrushSettings,
    pub view_mode: ViewMode,
    pub db: PigmentDb,
    stroke: Option<StrokeBuilder>,
    stroke_brush: BrushSettings,
    queued: Vec<Dab>,
    step_open: bool,
    stroke_surface: Surface,
    hold: Option<Hold>,
    motion_anchor: Option<MotionAnchor>,
    tube: Tube,
    drawer: DrawUp,
}

impl PaintSession {
    pub fn new(gpu: &GpuContext, spec: CanvasSpec) -> Result<PaintSession, CanvasError> {
        let tray = tray_spec(&spec.pigment_slots);
        let undo = UndoLimits {
            budget_bytes: history_budget(&spec),
            max_steps: HISTORY_MAX_STEPS,
        };
        PaintSession::assemble(gpu, spec, tray, undo)
    }

    /// A session for previews: the same simulation, but no undo history and a
    /// one-tile tray, so it costs little more than its canvas. Replacing its
    /// document gives a normal session, with a full tray and undo history.
    pub fn headless(gpu: &GpuContext, spec: CanvasSpec) -> Result<PaintSession, CanvasError> {
        let tray = CanvasSpec {
            width: TILE_SIZE,
            height: TILE_SIZE,
            ..tray_spec(&spec.pigment_slots)
        };
        let undo = UndoLimits {
            budget_bytes: 0,
            max_steps: 0,
        };
        PaintSession::assemble(gpu, spec, tray, undo)
    }

    fn assemble(
        gpu: &GpuContext,
        spec: CanvasSpec,
        tray: CanvasSpec,
        undo: UndoLimits,
    ) -> Result<PaintSession, CanvasError> {
        let db = PigmentDb::builtin();
        let tray = Canvas::new(gpu, &db, tray)?;
        let canvas = Canvas::new(gpu, &db, spec)?;
        let mut session = PaintSession {
            canvas,
            tray,
            engine: WetOilEngine::new(gpu),
            renderer: Renderer::new(gpu),
            presenter: Presenter::new(gpu),
            // After the canvases: the undo atlas shrinks to fit what they leave.
            history: History::new(gpu, undo.budget_bytes, undo.max_steps),
            brush: BrushSettings::default(),
            view_mode: ViewMode::Color,
            db,
            stroke: None,
            stroke_brush: BrushSettings::default(),
            queued: Vec::new(),
            step_open: false,
            stroke_surface: Surface::Painting,
            hold: None,
            motion_anchor: None,
            tube: Tube::new(gpu),
            drawer: DrawUp::new(gpu),
        };
        session.reload_brush(gpu);
        Ok(session)
    }

    pub fn canvas_of(&self, surface: Surface) -> &Canvas {
        surface_canvas(surface, &self.canvas, &self.tray)
    }

    pub fn canvas_of_mut(&mut self, surface: Surface) -> &mut Canvas {
        surface_canvas(surface, &mut self.canvas, &mut self.tray)
    }

    pub fn add_slot(&mut self, gpu: &GpuContext, id: &str) -> Result<usize, String> {
        let slot = self.canvas.add_slot(gpu, &self.db, id)?;
        self.tray.add_slot(gpu, &self.db, id)?;
        Ok(slot)
    }

    pub fn encode_frame(
        &mut self,
        gpu: &GpuContext,
        frame_clock_ms: f64,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.encode_dabs_or_settle(gpu, Surface::Painting, frame_clock_ms, encoder);
        self.encode_painting_shade(gpu, encoder);
    }

    pub(crate) fn encode_still_frame(
        &mut self,
        gpu: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.encode_queued_dabs(gpu, encoder);
        self.encode_painting_shade(gpu, encoder);
    }

    fn encode_painting_shade(&mut self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        self.renderer.encode_shade(
            gpu,
            &mut self.canvas,
            ShadeRequest {
                params: self.engine.params(),
                mode: self.view_mode,
            },
            encoder,
        );
    }

    pub fn encode_tray_frame(
        &mut self,
        gpu: &GpuContext,
        frame_clock_ms: f64,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.encode_dabs_or_settle(gpu, Surface::Tray, frame_clock_ms, encoder);
        self.renderer.encode_shade(
            gpu,
            &mut self.tray,
            ShadeRequest {
                params: self.engine.params(),
                mode: ViewMode::Color,
            },
            encoder,
        );
    }

    pub fn undo(&mut self, gpu: &GpuContext) -> bool {
        self.settle_pending_step(gpu);
        self.history.commit_step();
        let Some(surface) = self.history.undo_surface() else {
            return false;
        };
        let canvas = surface_canvas(surface, &mut self.canvas, &mut self.tray);
        self.history.undo(gpu, canvas)
    }

    pub fn redo(&mut self, gpu: &GpuContext) -> bool {
        self.settle_pending_step(gpu);
        let Some(surface) = self.history.redo_surface() else {
            return false;
        };
        let canvas = surface_canvas(surface, &mut self.canvas, &mut self.tray);
        self.history.redo(gpu, canvas)
    }

    pub fn pick_colour(
        &mut self,
        gpu: &GpuContext,
        x: f32,
        y: f32,
    ) -> Result<Option<Mixture>, GpuLost> {
        self.pick_colour_on(gpu, Surface::Painting, x, y)
    }

    pub fn pick_colour_on(
        &mut self,
        gpu: &GpuContext,
        surface: Surface,
        x: f32,
        y: f32,
    ) -> Result<Option<Mixture>, GpuLost> {
        let mut encoder = new_encoder(gpu, "pick_colour");
        self.encode_queued_dabs(gpu, &mut encoder);
        let request = PickRequest {
            center: [x, y],
            radius: PICK_RADIUS_PX,
        };
        self.engine
            .encode_eyedropper(gpu, self.canvas_of(surface), request, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        self.engine.picked_mixture(gpu)
    }
}

fn surface_canvas<C>(surface: Surface, painting: C, tray: C) -> C {
    match surface {
        Surface::Painting => painting,
        Surface::Tray => tray,
    }
}

fn tile_count(width: u32, height: u32) -> u64 {
    u64::from(width.div_ceil(TILE_SIZE)) * u64::from(height.div_ceil(TILE_SIZE))
}

fn history_budget(spec: &CanvasSpec) -> u64 {
    let painting = tile_count(spec.width, spec.height) * HISTORY_PAINTING_COPIES;
    let tray = tile_count(TRAY_WIDTH, TRAY_HEIGHT) * HISTORY_TRAY_COPIES;
    HISTORY_BUDGET_BYTES.min((painting + tray) * bytes_per_tile_all_textures())
}
