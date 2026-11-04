use std::collections::BTreeSet;

use crate::brush::BrushSettings;
use crate::canvas::{Canvas, CanvasError, CanvasSpec, TILE_SIZE, TileCoord, tiles_in_rect};
use crate::engine::{BrushReadout, PaintEngine, WetOilEngine};
use crate::gpu::{GpuContext, GpuLost};
use crate::history::{History, bytes_per_tile_all_textures};
use crate::pigments::{Mixture, PigmentDb};
use crate::present::Presenter;
use crate::render::{Renderer, ViewMode};
use crate::stroke::{Dab, StrokeBuilder, StrokeSample, Tool};

const DRY_TILES_PER_SUBMIT: usize = 64;
const PICK_RADIUS_PX: f32 = 4.0;
const HISTORY_BUDGET_BYTES: u64 = 512 * 1024 * 1024;
const HISTORY_MAX_STEPS: usize = 100;
const HISTORY_CANVAS_COPIES: u64 = 4;
const PLACEHOLDER_SIZE_PX: u32 = 1;
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
    pub fn new(gpu: &GpuContext, spec: CanvasSpec) -> Result<PaintSession, CanvasError> {
        let db = PigmentDb::builtin();
        let history_budget = history_budget(&spec);
        let canvas = Canvas::new(gpu, &db, spec)?;
        let mut session = PaintSession {
            canvas,
            engine: WetOilEngine::new(gpu),
            renderer: Renderer::new(gpu),
            presenter: Presenter::new(gpu),
            history: History::new(gpu, history_budget, HISTORY_MAX_STEPS),
            brush: BrushSettings::default(),
            view_mode: ViewMode::Color,
            db,
            stroke: None,
            stroke_brush: BrushSettings::default(),
            queued: Vec::new(),
            step_open: false,
        };
        session.reload_brush(gpu);
        Ok(session)
    }

    pub fn replace_document(
        &mut self,
        gpu: &GpuContext,
        spec: CanvasSpec,
    ) -> Result<(), CanvasError> {
        self.stroke = None;
        self.queued.clear();
        self.step_open = false;
        self.history.clear();
        let previous = self.canvas.spec.clone();
        let placeholder = CanvasSpec {
            width: PLACEHOLDER_SIZE_PX,
            height: PLACEHOLDER_SIZE_PX,
            ..previous.clone()
        };
        let placeholder = Canvas::new(gpu, &self.db, placeholder)?;
        drop(std::mem::replace(&mut self.canvas, placeholder));
        gpu.queue.submit([]);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let replaced = match Canvas::new(gpu, &self.db, spec) {
            Ok(canvas) => {
                self.canvas = canvas;
                Ok(())
            }
            Err(error) => {
                if let Ok(blank) = Canvas::new(gpu, &self.db, previous) {
                    self.canvas = blank;
                }
                Err(error)
            }
        };
        self.history.refit(gpu, history_budget(&self.canvas.spec));
        self.view_mode = ViewMode::Color;
        self.engine.clean_brush(gpu);
        self.engine.clean_knife(gpu);
        replaced
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

    pub fn pick_colour(
        &mut self,
        gpu: &GpuContext,
        x: f32,
        y: f32,
    ) -> Result<Option<Mixture>, GpuLost> {
        let mut encoder = new_encoder(gpu, "pick_colour");
        self.encode_queued_dabs(gpu, &mut encoder);
        self.engine
            .encode_eyedropper(gpu, &self.canvas, x, y, PICK_RADIUS_PX, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        self.engine.picked_mixture(gpu)
    }

    pub fn resize_tip(&mut self) {
        self.engine
            .resize_tip(self.brush.geometry.max_diameter_px / 2.0);
    }

    pub fn reload_brush(&mut self, gpu: &GpuContext) {
        self.flush(gpu);
        self.resize_tip();
        let mixture = self.recipe_mixture();
        if is_pigment_free(&mixture) {
            self.engine.clean_brush(gpu);
        } else {
            self.engine.load_reservoir(gpu, &mixture);
        }
    }

    pub fn dip(&mut self, gpu: &GpuContext, mixture: &Mixture, amount: f32) -> Result<(), GpuLost> {
        self.flush(gpu);
        if self.holds_knife() {
            self.resize_tip();
            self.engine.dip_knife(gpu, mixture, amount)
        } else {
            self.engine.dip(gpu, mixture, amount);
            Ok(())
        }
    }

    pub fn restore_reserve(&mut self, gpu: &GpuContext) {
        self.flush(gpu);
        let mixture = self.recipe_mixture();
        if !is_pigment_free(&mixture) {
            self.engine.restore_reserve(gpu, &mixture);
        }
    }

    pub fn tip_mean_mixture(&mut self, gpu: &GpuContext) -> Result<Mixture, GpuLost> {
        self.flush(gpu);
        self.engine.tip_mean_mixture(gpu)
    }

    pub fn read_brush(&mut self, gpu: &GpuContext) -> Result<BrushReadout, GpuLost> {
        self.flush(gpu);
        self.engine.read_brush(gpu)
    }

    pub fn clean_brush(&mut self, gpu: &GpuContext) {
        self.flush(gpu);
        if self.holds_knife() {
            self.engine.clean_knife(gpu);
        } else {
            self.engine.clean_brush(gpu);
        }
    }

    fn holds_knife(&self) -> bool {
        self.brush.tool == Tool::Knife
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

    fn recipe_mixture(&self) -> Mixture {
        self.brush
            .reservoir_mixture(self.canvas.spec.pigment_slots.len())
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

fn history_budget(spec: &CanvasSpec) -> u64 {
    let tiles =
        u64::from(spec.width.div_ceil(TILE_SIZE)) * u64::from(spec.height.div_ceil(TILE_SIZE));
    HISTORY_BUDGET_BYTES.min(tiles * HISTORY_CANVAS_COPIES * bytes_per_tile_all_textures())
}

fn is_pigment_free(mixture: &Mixture) -> bool {
    mixture.fractions.iter().all(|fraction| *fraction <= 0.0)
}

fn new_encoder(gpu: &GpuContext, label: &str) -> wgpu::CommandEncoder {
    gpu.device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) })
}
