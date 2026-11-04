use crate::canvas::{Canvas, CanvasError, CanvasSpec};
use crate::gpu::GpuContext;
use crate::render::ViewMode;
use crate::surface::tray_spec;

use super::{PaintSession, history_budget};

const PLACEHOLDER_SIZE_PX: u32 = 1;

impl PaintSession {
    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
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
}

fn placeholder_spec(spec: &CanvasSpec) -> CanvasSpec {
    CanvasSpec {
        width: PLACEHOLDER_SIZE_PX,
        height: PLACEHOLDER_SIZE_PX,
        ..spec.clone()
    }
}
