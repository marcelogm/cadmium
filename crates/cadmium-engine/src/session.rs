use std::collections::VecDeque;

use crate::brush::BrushSettings;
use crate::canvas::{Canvas, CanvasError, CanvasSpec, TILE_SIZE};
use crate::engine::{PaintEngine, PickRequest, WetOilEngine};
use crate::gpu::{GpuContext, GpuLost, new_encoder};
use crate::history::{
    HELD_PAINT_HISTORY_BUDGET_BYTES, History, HistoryLimits, bytes_per_tile_all_textures,
};
use crate::palette::Palette;
use crate::pigments::{Mixture, PigmentDb};
use crate::present::Presenter;
use crate::render::{Renderer, ShadeRequest, ViewMode};
use crate::stroke::StrokeBuilder;

mod brush;
mod command;
mod queue;
mod removal;
mod replacement;
mod settle;
mod stroke;
mod walk;

use queue::Queued;
use settle::MotionAnchor;

const PICK_RADIUS_PX: f32 = 4.0;
const HISTORY_BUDGET_BYTES: u64 = 512 * 1024 * 1024;
const HISTORY_MAX_STEPS: usize = 100;
const HISTORY_PAINTING_COPIES: u64 = 4;

pub struct PaintSession {
    pub canvas: Canvas,
    pub engine: WetOilEngine,
    pub renderer: Renderer,
    pub presenter: Presenter,
    pub history: History,
    pub brush: BrushSettings,
    pub view_mode: ViewMode,
    pub db: PigmentDb,
    pub palette: Palette,
    stroke: Option<StrokeBuilder>,
    queued: VecDeque<Queued>,
    frame_dab_limit: Option<usize>,
    last_frame_dabs: usize,
    motion_anchor: Option<MotionAnchor>,
}

impl PaintSession {
    pub fn new(gpu: &GpuContext, spec: CanvasSpec) -> Result<PaintSession, CanvasError> {
        let undo = HistoryLimits {
            tile_budget_bytes: history_budget(&spec),
            max_steps: HISTORY_MAX_STEPS,
            held_paint_budget_bytes: HELD_PAINT_HISTORY_BUDGET_BYTES,
        };
        PaintSession::assemble(gpu, spec, undo)
    }

    /// A session for previews: the same simulation, but no undo history, so it costs
    /// little more than its canvas. It stays without undo even if its document is
    /// replaced.
    pub fn headless(gpu: &GpuContext, spec: CanvasSpec) -> Result<PaintSession, CanvasError> {
        let undo = HistoryLimits {
            tile_budget_bytes: 0,
            max_steps: 0,
            held_paint_budget_bytes: 0,
        };
        PaintSession::assemble(gpu, spec, undo)
    }

    fn assemble(
        gpu: &GpuContext,
        spec: CanvasSpec,
        undo: HistoryLimits,
    ) -> Result<PaintSession, CanvasError> {
        let db = PigmentDb::builtin();
        let palette = Palette::starting(&db, &spec.pigment_slots);
        let canvas = Canvas::new(gpu, &db, spec)?;
        let mut session = PaintSession {
            canvas,
            engine: WetOilEngine::new(gpu),
            renderer: Renderer::new(gpu),
            presenter: Presenter::new(gpu),
            history: History::new(gpu, undo),
            brush: BrushSettings::default(),
            view_mode: ViewMode::Color,
            db,
            palette,
            stroke: None,
            queued: VecDeque::new(),
            frame_dab_limit: None,
            last_frame_dabs: 0,
            motion_anchor: None,
        };
        session.reload_brush(gpu);
        Ok(session)
    }

    pub fn add_slot(&mut self, gpu: &GpuContext, id: &str) -> Result<usize, String> {
        self.canvas.add_slot(gpu, &self.db, id)
    }

    /// Paints what this frame may of the queue, then shades the painting. The encoder must
    /// be submitted before the next frame is encoded: a knife load waiting in the queue
    /// reads the blade back at the start of a frame and must see the frames before it.
    pub fn encode_frame(
        &mut self,
        gpu: &GpuContext,
        frame_clock_ms: f64,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.encode_dabs_or_settle(gpu, frame_clock_ms, encoder);
        self.encode_painting_shade(gpu, encoder);
    }

    pub(crate) fn encode_still_frame(
        &mut self,
        gpu: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.flush(gpu);
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

    pub fn undo(&mut self, gpu: &GpuContext) -> bool {
        self.settle_pending_step(gpu);
        self.history.undo(gpu, &mut self.canvas, &mut self.engine)
    }

    pub fn redo(&mut self, gpu: &GpuContext) -> bool {
        self.settle_pending_step(gpu);
        self.history.redo(gpu, &mut self.canvas, &mut self.engine)
    }

    pub fn pick_colour(
        &mut self,
        gpu: &GpuContext,
        x: f32,
        y: f32,
    ) -> Result<Option<Mixture>, GpuLost> {
        self.flush(gpu);
        let mut encoder = new_encoder(gpu, "pick_colour");
        let request = PickRequest {
            center: [x, y],
            radius: PICK_RADIUS_PX,
        };
        self.engine
            .encode_eyedropper(gpu, &self.canvas, request, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        self.engine.picked_mixture(gpu)
    }
}

fn tile_count(width: u32, height: u32) -> u64 {
    u64::from(width.div_ceil(TILE_SIZE)) * u64::from(height.div_ceil(TILE_SIZE))
}

fn history_budget(spec: &CanvasSpec) -> u64 {
    HISTORY_BUDGET_BYTES.min(
        tile_count(spec.width, spec.height)
            * HISTORY_PAINTING_COPIES
            * bytes_per_tile_all_textures(),
    )
}
