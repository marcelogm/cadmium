use std::collections::BTreeSet;

use crate::brush::BrushSettings;
use crate::canvas::{Canvas, CanvasSpec, TileCoord, tiles_in_rect};
use crate::engine::{PaintEngine, WetOilEngine};
use crate::gpu::GpuContext;
use crate::history::History;
use crate::pigments::{Mixture, PigmentDb};
use crate::present::Presenter;
use crate::render::{Renderer, ViewMode};
use crate::stroke::{Dab, StrokeBuilder, StrokeSample};

const DRY_TILES_PER_SUBMIT: usize = 64;
const PICK_RADIUS_PX: f32 = 4.0;
const HISTORY_BUDGET_BYTES: u64 = 512 * 1024 * 1024;
const HISTORY_MAX_STEPS: usize = 100;
const SAVE_MARGIN_PX: f32 = 4.0;

pub struct PaintSession {
    pub canvas: Canvas,
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
}

impl PaintSession {
    pub fn new(gpu: &GpuContext, spec: CanvasSpec) -> PaintSession {
        let db = PigmentDb::builtin();
        let canvas = Canvas::new(gpu, &db, spec);
        let mut session = PaintSession {
            canvas,
            engine: WetOilEngine::new(gpu),
            renderer: Renderer::new(gpu),
            presenter: Presenter::new(gpu),
            history: History::new(gpu, HISTORY_BUDGET_BYTES, HISTORY_MAX_STEPS),
            brush: BrushSettings::default(),
            view_mode: ViewMode::Color,
            db,
            stroke: None,
            stroke_brush: BrushSettings::default(),
            queued: Vec::new(),
            step_open: false,
        };
        session.reload_brush(gpu);
        session
    }

    pub fn begin_stroke(&mut self, gpu: &GpuContext, sample: StrokeSample) {
        self.settle_pending_step(gpu);
        self.history.begin_step();
        self.step_open = true;
        self.stroke_brush = self.brush.clone();
        self.engine
            .begin_stroke(gpu, &self.canvas, &self.stroke_brush);
        let mut builder = StrokeBuilder::new(self.stroke_brush.geometry);
        self.queued.extend(builder.push(sample));
        self.stroke = Some(builder);
    }

    pub fn add_samples(&mut self, samples: &[StrokeSample]) {
        let Some(builder) = self.stroke.as_mut() else {
            return;
        };
        for sample in samples {
            self.queued.extend(builder.push(*sample));
        }
    }

    pub fn end_stroke(&mut self, gpu: &GpuContext) {
        let Some(mut builder) = self.stroke.take() else {
            return;
        };
        self.queued.extend(builder.finish());
        self.engine.end_stroke(gpu, &self.canvas);
        if self.queued.is_empty() {
            self.commit_open_step();
        }
    }

    pub fn encode_frame(&mut self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        self.encode_queued_dabs(gpu, encoder);
        self.renderer.encode_shade(
            gpu,
            &mut self.canvas,
            self.engine.params(),
            self.view_mode,
            encoder,
        );
    }

    pub fn dry(&mut self, gpu: &GpuContext) -> bool {
        self.settle_pending_step(gpu);
        let tiles = self.canvas.wet_tiles.take();
        self.history.begin_drying_step(tiles.len());
        for chunk in tiles.chunks(DRY_TILES_PER_SUBMIT) {
            let mut encoder = new_encoder(gpu, "dry");
            self.history
                .encode_save_tiles(gpu, &self.canvas, chunk, &mut encoder);
            self.engine
                .encode_dry_tiles(gpu, &mut self.canvas, chunk, &mut encoder);
            gpu.queue.submit([encoder.finish()]);
        }
        self.history.commit_step()
    }

    pub fn undo(&mut self, gpu: &GpuContext) -> bool {
        self.settle_pending_step(gpu);
        self.history.undo(gpu, &mut self.canvas)
    }

    pub fn redo(&mut self, gpu: &GpuContext) -> bool {
        self.settle_pending_step(gpu);
        self.history.redo(gpu, &mut self.canvas)
    }

    pub fn pick_color(&mut self, gpu: &GpuContext, x: f32, y: f32) -> Mixture {
        let mut encoder = new_encoder(gpu, "pick_color");
        self.encode_queued_dabs(gpu, &mut encoder);
        self.engine
            .encode_eyedropper(gpu, &self.canvas, x, y, PICK_RADIUS_PX, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        self.engine.reservoir_mixture(gpu)
    }

    pub fn reload_brush(&mut self, gpu: &GpuContext) {
        let mixture = self
            .brush
            .reservoir_mixture(self.canvas.spec.pigment_slots.len());
        self.engine
            .set_tip_radius(self.brush.geometry.max_diameter_px / 2.0);
        self.engine.load_reservoir(gpu, &mixture);
    }

    pub fn clean_brush(&mut self, gpu: &GpuContext) {
        self.engine.clean_brush(gpu);
    }

    pub fn has_pending_work(&self) -> bool {
        !self.queued.is_empty()
            || !self.canvas.dirty.is_empty()
            || (self.step_open && self.stroke.is_none())
    }

    pub fn flush(&mut self, gpu: &GpuContext) {
        let step_awaits_commit = self.step_open && self.stroke.is_none();
        if self.queued.is_empty() && !step_awaits_commit {
            return;
        }
        let mut encoder = new_encoder(gpu, "flush_queued_dabs");
        self.encode_queued_dabs(gpu, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
    }

    fn settle_pending_step(&mut self, gpu: &GpuContext) {
        self.end_stroke(gpu);
        self.flush(gpu);
    }

    fn encode_queued_dabs(&mut self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        if !self.queued.is_empty() {
            let touched = self.tiles_touched_by_queued_dabs();
            self.history
                .encode_save_tiles(gpu, &self.canvas, &touched, encoder);
            self.engine.encode_dabs(
                gpu,
                &mut self.canvas,
                &self.stroke_brush,
                &self.queued,
                encoder,
            );
            self.queued.clear();
        }
        if self.stroke.is_none() {
            self.commit_open_step();
        }
    }

    fn commit_open_step(&mut self) {
        if self.step_open {
            self.history.commit_step();
            self.step_open = false;
        }
    }

    fn tiles_touched_by_queued_dabs(&self) -> Vec<TileCoord> {
        let width = self.canvas.spec.width;
        let height = self.canvas.spec.height;
        let mut touched = BTreeSet::new();
        for dab in &self.queued {
            let Some([x0, y0, x1, y1]) =
                self.engine
                    .dab_bounds(&self.canvas, &self.stroke_brush, dab)
            else {
                continue;
            };
            touched.extend(tiles_in_rect(
                width,
                height,
                x0 - SAVE_MARGIN_PX,
                y0 - SAVE_MARGIN_PX,
                x1 + SAVE_MARGIN_PX,
                y1 + SAVE_MARGIN_PX,
            ));
        }
        touched.into_iter().collect()
    }
}

fn new_encoder(gpu: &GpuContext, label: &str) -> wgpu::CommandEncoder {
    gpu.device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) })
}
