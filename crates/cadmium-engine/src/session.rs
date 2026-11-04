use std::collections::BTreeSet;

use crate::brush::BrushSettings;
use crate::canvas::{
    Canvas, CanvasError, CanvasSpec, StateTexture, TILE_SIZE, TileCoord, tiles_in_rect,
};
use crate::engine::{BrushOutline, BrushReadout, PaintEngine, WetOilEngine};
use crate::gpu::{GpuContext, GpuLost};
use crate::history::{History, bytes_per_tile_all_textures};
use crate::pigments::{Mixture, PigmentDb};
use crate::present::Presenter;
use crate::render::{Renderer, ViewMode};
use crate::stroke::{Dab, StrokeBuilder, StrokeSample, Tool};
use crate::surface::{Surface, TRAY_HEIGHT, TRAY_WIDTH, tray_spec};
use crate::tube::{Nozzle, Tube, squeeze_region};

const DRY_TILES_PER_SUBMIT: usize = 64;
const PICK_RADIUS_PX: f32 = 4.0;
const HISTORY_BUDGET_BYTES: u64 = 512 * 1024 * 1024;
const HISTORY_MAX_STEPS: usize = 100;
const HISTORY_PAINTING_COPIES: u64 = 4;
const HISTORY_TRAY_COPIES: u64 = 1;
const PLACEHOLDER_SIZE_PX: u32 = 1;
const SAVE_MARGIN_PX: f32 = 4.0;

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
    hold: Option<Surface>,
    tube: Tube,
}

impl PaintSession {
    pub fn new(gpu: &GpuContext, spec: CanvasSpec) -> Result<PaintSession, CanvasError> {
        let db = PigmentDb::builtin();
        let history_budget = history_budget(&spec);
        let tray = Canvas::new(gpu, &db, tray_spec(&spec.pigment_slots))?;
        let canvas = Canvas::new(gpu, &db, spec)?;
        let mut session = PaintSession {
            canvas,
            tray,
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
            stroke_surface: Surface::Painting,
            hold: None,
            tube: Tube::new(gpu),
        };
        session.reload_brush(gpu);
        Ok(session)
    }

    pub fn replace_document(
        &mut self,
        gpu: &GpuContext,
        spec: CanvasSpec,
    ) -> Result<(), CanvasError> {
        self.end_hold();
        self.stroke = None;
        self.queued.clear();
        self.step_open = false;
        self.history.clear();
        let previous = self.canvas.spec.clone();
        let placeholder = Canvas::new(gpu, &self.db, placeholder_spec(&previous))?;
        let tray_placeholder = Canvas::new(gpu, &self.db, placeholder_spec(&self.tray.spec))?;
        drop(std::mem::replace(&mut self.canvas, placeholder));
        drop(std::mem::replace(&mut self.tray, tray_placeholder));
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
        let painting_tray = tray_spec(&self.canvas.spec.pigment_slots);
        let tray_rebuilt = match Canvas::new(gpu, &self.db, painting_tray.clone()) {
            Ok(tray) => {
                self.tray = tray;
                Ok(())
            }
            Err(error) => {
                if let Ok(blank) = Canvas::new(gpu, &self.db, placeholder_spec(&painting_tray)) {
                    self.tray = blank;
                }
                Err(error)
            }
        };
        self.history.refit(gpu, history_budget(&self.canvas.spec));
        self.view_mode = ViewMode::Color;
        self.engine.clean_brush(gpu);
        self.engine.clean_knife(gpu);
        replaced.and(tray_rebuilt)
    }

    pub fn canvas_of(&self, surface: Surface) -> &Canvas {
        match surface {
            Surface::Painting => &self.canvas,
            Surface::Tray => &self.tray,
        }
    }

    pub fn canvas_of_mut(&mut self, surface: Surface) -> &mut Canvas {
        surface_canvas(surface, &mut self.canvas, &mut self.tray)
    }

    pub fn add_slot(&mut self, gpu: &GpuContext, id: &str) -> Result<usize, String> {
        let slot = self.canvas.add_slot(gpu, &self.db, id)?;
        self.tray.add_slot(gpu, &self.db, id)?;
        Ok(slot)
    }

    pub fn begin_stroke(&mut self, gpu: &GpuContext, sample: StrokeSample) {
        self.begin_stroke_on(gpu, Surface::Painting, sample);
    }

    pub fn begin_stroke_on(&mut self, gpu: &GpuContext, surface: Surface, sample: StrokeSample) {
        self.settle_pending_step(gpu);
        self.history.begin_step(surface);
        self.stroke_surface = surface;
        self.step_open = true;
        self.stroke_brush = self.brush.clone();
        self.engine.begin_stroke();
        let mut builder = StrokeBuilder::new(self.stroke_brush.stroke_geometry());
        self.queued.extend(builder.push(sample));
        self.stroke = Some(builder);
    }

    pub fn begin_hold(&mut self, gpu: &GpuContext, surface: Surface) {
        self.settle_pending_step(gpu);
        self.history.begin_step(surface);
        self.hold = Some(surface);
    }

    pub fn end_hold(&mut self) {
        if self.hold.take().is_some() {
            self.history.commit_step();
        }
    }

    pub fn is_holding(&self) -> bool {
        self.hold.is_some()
    }

    pub fn squeeze(&mut self, gpu: &GpuContext, nozzle: Nozzle) {
        let Some(surface) = self.hold else {
            return;
        };
        let canvas = surface_canvas(surface, &mut self.canvas, &mut self.tray);
        let params = self.engine.params();
        let Some(region) = squeeze_region(canvas, params, &nozzle) else {
            return;
        };
        let [x0, y0, x1, y1] = region.rect();
        let tiles = tiles_in_rect(canvas.spec.width, canvas.spec.height, x0, y0, x1, y1);
        let mut encoder = new_encoder(gpu, "squeeze");
        self.history
            .encode_save_tiles(gpu, canvas, &tiles, &mut encoder);
        self.tube
            .encode_squeeze(gpu, canvas, params, &nozzle, &region, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
    }

    pub fn add_samples(&mut self, samples: &[StrokeSample]) {
        let Some(builder) = self.stroke.as_mut() else {
            return;
        };
        for sample in samples {
            self.queued.extend(builder.push(*sample));
        }
    }

    pub fn end_stroke(&mut self) {
        let Some(mut builder) = self.stroke.take() else {
            return;
        };
        self.queued.extend(builder.finish());
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

    pub fn encode_tray_frame(&mut self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        self.encode_queued_dabs(gpu, encoder);
        self.renderer.encode_shade(
            gpu,
            &mut self.tray,
            self.engine.params(),
            ViewMode::Color,
            encoder,
        );
    }

    pub fn dry(&mut self, gpu: &GpuContext) -> bool {
        self.settle_pending_step(gpu);
        let tiles = self.canvas.wet_tiles.take();
        self.history
            .begin_removal_step(Surface::Painting, tiles.len());
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

    pub fn clear_tray(&mut self, gpu: &GpuContext) -> ClearOutcome {
        self.settle_pending_step(gpu);
        let tiles = self.tray.wet_tiles.take();
        if tiles.is_empty() {
            return ClearOutcome::NothingWet;
        }
        self.history.begin_removal_step(Surface::Tray, tiles.len());
        let mut encoder = new_encoder(gpu, "clear_tray");
        self.history
            .encode_save_tiles(gpu, &self.tray, &tiles, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        let texels = (self.tray.spec.width * self.tray.spec.height) as usize;
        self.tray
            .write_f32(gpu, StateTexture::WetVol, &vec![0.0; texels]);
        self.tray.dirty.mark_all();
        ClearOutcome::Cleared {
            undoable: self.history.commit_step(),
        }
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

    pub fn brush_outline(&self, pen: &StrokeSample) -> Option<BrushOutline> {
        self.engine.brush_outline(&self.brush, pen)
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
        self.engine.encode_eyedropper(
            gpu,
            self.canvas_of(surface),
            x,
            y,
            PICK_RADIUS_PX,
            &mut encoder,
        );
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
        if mixture.is_empty() {
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
        if !mixture.is_empty() {
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
        !self.queued.is_empty() || !self.canvas.dirty.is_empty() || self.step_awaits_commit()
    }

    pub fn tray_has_pending_work(&self) -> bool {
        !self.queued.is_empty() || !self.tray.dirty.is_empty() || self.step_awaits_commit()
    }

    fn step_awaits_commit(&self) -> bool {
        self.step_open && self.stroke.is_none()
    }

    pub fn flush(&mut self, gpu: &GpuContext) {
        if self.queued.is_empty() && !self.step_awaits_commit() {
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
        self.end_hold();
        self.end_stroke();
        self.flush(gpu);
    }

    fn encode_queued_dabs(&mut self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        if !self.queued.is_empty() {
            let touched = self.tiles_touched_by_queued_dabs();
            let canvas = surface_canvas(self.stroke_surface, &mut self.canvas, &mut self.tray);
            self.history
                .encode_save_tiles(gpu, canvas, &touched, encoder);
            self.engine
                .encode_dabs(gpu, canvas, &self.stroke_brush, &self.queued, encoder);
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
        let canvas = self.canvas_of(self.stroke_surface);
        let width = canvas.spec.width;
        let height = canvas.spec.height;
        let mut touched = BTreeSet::new();
        for dab in &self.queued {
            let Some([x0, y0, x1, y1]) = self.engine.dab_bounds(canvas, &self.stroke_brush, dab)
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

fn surface_canvas<'a>(
    surface: Surface,
    painting: &'a mut Canvas,
    tray: &'a mut Canvas,
) -> &'a mut Canvas {
    match surface {
        Surface::Painting => painting,
        Surface::Tray => tray,
    }
}

fn placeholder_spec(spec: &CanvasSpec) -> CanvasSpec {
    CanvasSpec {
        width: PLACEHOLDER_SIZE_PX,
        height: PLACEHOLDER_SIZE_PX,
        ..spec.clone()
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

fn new_encoder(gpu: &GpuContext, label: &str) -> wgpu::CommandEncoder {
    gpu.device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) })
}
