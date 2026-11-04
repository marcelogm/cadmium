use crate::canvas::{Canvas, CanvasError, CanvasSpec};
use crate::gpu::GpuContext;
use crate::pigments::PigmentDb;
use crate::render::ViewMode;
use crate::surface::tray_spec;

use super::{PaintSession, history_budget};

const PLACEHOLDER_SIZE_PX: u32 = 1;

impl PaintSession {
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
        let previous = self.swap_in_placeholders(gpu)?;
        let replaced = self.replace_canvas(gpu, spec, previous);
        let tray_rebuilt = self.rebuild_tray(gpu);
        self.history.refit(gpu, history_budget(&self.canvas.spec));
        self.view_mode = ViewMode::Color;
        self.engine.clean_brush(gpu);
        self.engine.clean_knife(gpu);
        replaced.and(tray_rebuilt)
    }

    fn swap_in_placeholders(&mut self, gpu: &GpuContext) -> Result<CanvasSpec, CanvasError> {
        let previous = self.canvas.spec.clone();
        let placeholder = Canvas::new(gpu, &self.db, placeholder_spec(&previous))?;
        let tray_placeholder = Canvas::new(gpu, &self.db, placeholder_spec(&self.tray.spec))?;
        drop(std::mem::replace(&mut self.canvas, placeholder));
        drop(std::mem::replace(&mut self.tray, tray_placeholder));
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
        let (canvas, replaced) = new_canvas_or_blank(gpu, &self.db, spec, previous);
        if let Some(canvas) = canvas {
            self.canvas = canvas;
        }
        replaced
    }

    fn rebuild_tray(&mut self, gpu: &GpuContext) -> Result<(), CanvasError> {
        let painting_tray = tray_spec(&self.canvas.spec.pigment_slots);
        let blank = placeholder_spec(&painting_tray);
        let (tray, rebuilt) = new_canvas_or_blank(gpu, &self.db, painting_tray, blank);
        if let Some(tray) = tray {
            self.tray = tray;
        }
        rebuilt
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

fn placeholder_spec(spec: &CanvasSpec) -> CanvasSpec {
    CanvasSpec {
        width: PLACEHOLDER_SIZE_PX,
        height: PLACEHOLDER_SIZE_PX,
        ..spec.clone()
    }
}
