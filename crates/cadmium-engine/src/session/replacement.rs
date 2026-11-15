use crate::canvas::{Canvas, CanvasError, CanvasSpec};
use crate::gpu::GpuContext;
use crate::palette::Palette;
use crate::pigments::PigmentDb;
use crate::render::ViewMode;

use super::{PaintSession, history_budget};

const PLACEHOLDER_SIZE_PX: u32 = 1;

impl PaintSession {
    pub fn replace_document(
        &mut self,
        gpu: &GpuContext,
        spec: CanvasSpec,
    ) -> Result<(), CanvasError> {
        self.stroke = None;
        self.queued.clear();
        self.history.clear();
        let previous = self.swap_in_placeholder(gpu)?;
        let replaced = self.replace_canvas(gpu, spec, previous);
        self.palette = Palette::starting(&self.db, &self.canvas.spec.pigment_slots);
        self.history.refit(gpu, history_budget(&self.canvas.spec));
        self.view_mode = ViewMode::Color;
        self.engine.clean_brush(gpu);
        self.engine.clean_knife(gpu);
        replaced
    }

    fn swap_in_placeholder(&mut self, gpu: &GpuContext) -> Result<CanvasSpec, CanvasError> {
        let previous = self.canvas.spec.clone();
        let placeholder = Canvas::new(gpu, &self.db, placeholder_spec(&previous))?;
        drop(std::mem::replace(&mut self.canvas, placeholder));
        gpu.queue.submit([]);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        Ok(previous)
    }

    fn replace_canvas(
        &mut self,
        gpu: &GpuContext,
        spec: CanvasSpec,
        previous: CanvasSpec,
    ) -> Result<(), CanvasError> {
        let built = new_canvas_or_blank(gpu, &self.db, spec, previous);
        replace_if_built(&mut self.canvas, built)
    }
}

fn new_canvas_or_blank(
    gpu: &GpuContext,
    db: &PigmentDb,
    spec: CanvasSpec,
    blank: CanvasSpec,
) -> (Option<Canvas>, Result<(), CanvasError>) {
    match Canvas::new(gpu, db, spec) {
        Ok(canvas) => (Some(canvas), Ok(())),
        Err(error) => (Canvas::new(gpu, db, blank).ok(), Err(error)),
    }
}

fn replace_if_built(
    target: &mut Canvas,
    (built, outcome): (Option<Canvas>, Result<(), CanvasError>),
) -> Result<(), CanvasError> {
    if let Some(canvas) = built {
        *target = canvas;
    }
    outcome
}

fn placeholder_spec(spec: &CanvasSpec) -> CanvasSpec {
    CanvasSpec {
        width: PLACEHOLDER_SIZE_PX,
        height: PLACEHOLDER_SIZE_PX,
        ..spec.clone()
    }
}
