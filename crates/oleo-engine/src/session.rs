use crate::brush::BrushSettings;
use crate::canvas::{Canvas, CanvasSpec};
use crate::engine::{PaintEngine, WetOilEngine};
use crate::gpu::GpuContext;
use crate::pigments::PigmentDb;
use crate::present::Presenter;
use crate::render::{Renderer, ViewMode};
use crate::stroke::{Dab, StrokeBuilder, StrokeSample};

pub struct PaintSession {
    pub canvas: Canvas,
    pub engine: WetOilEngine,
    pub renderer: Renderer,
    pub presenter: Presenter,
    pub brush: BrushSettings,
    pub view_mode: ViewMode,
    pub db: PigmentDb,
    stroke: Option<StrokeBuilder>,
    queued: Vec<Dab>,
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
            brush: BrushSettings::default(),
            view_mode: ViewMode::Color,
            db,
            stroke: None,
            queued: Vec::new(),
        };
        session.reload_brush(gpu);
        session
    }

    pub fn begin_stroke(&mut self, gpu: &GpuContext, sample: StrokeSample) {
        self.engine.begin_stroke(gpu, &self.canvas, &self.brush);
        let mut builder = StrokeBuilder::new(self.brush.geometry);
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
    }

    pub fn encode_frame(&mut self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        if !self.queued.is_empty() {
            self.engine
                .encode_dabs(gpu, &mut self.canvas, &self.brush, &self.queued, encoder);
            self.queued.clear();
        }
        self.renderer.encode_shade(
            gpu,
            &mut self.canvas,
            self.engine.params(),
            self.view_mode,
            encoder,
        );
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
        !self.queued.is_empty() || !self.canvas.dirty.is_empty()
    }
}
