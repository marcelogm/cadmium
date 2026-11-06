//! Paints the examples on the editor's own example sheet, one part per call, with the
//! painting's paint as it was when the editor opened (the dialog is modal: it can't change).

use std::rc::Rc;

use cadmium_engine::brush::BrushSpec;
use cadmium_engine::gpu::GpuContext;
use cadmium_studio::{ExampleRequest, ExampleSheet, StudioError, StudioPaint, Thumbnail};

use crate::state::AppState;

/// How one part of the painting went.
#[derive(Debug)]
pub(in crate::brush_editor) enum Tick {
    /// More parts to paint.
    Working,
    Done(Thumbnail),
    Failed(StudioError),
    /// Nothing was started.
    Idle,
}

pub(in crate::brush_editor) struct ExampleJob {
    gpu: Rc<GpuContext>,
    paint: StudioPaint,
    sheet: ExampleSheet,
}

impl ExampleJob {
    pub(in crate::brush_editor) fn new(state: &AppState) -> ExampleJob {
        ExampleJob {
            gpu: state.gpu.clone(),
            paint: state.studio_paint(),
            sheet: ExampleSheet::new(),
        }
    }

    /// Starts painting `brush`, dropping the painting under way.
    pub(in crate::brush_editor) fn start(&mut self, brush: &BrushSpec) -> Result<(), StudioError> {
        let request = ExampleRequest {
            brush,
            paint: &self.paint,
        };
        self.sheet.begin(&self.gpu, request)
    }

    /// Paints the next part.
    pub(in crate::brush_editor) fn tick(&mut self) -> Tick {
        if !self.sheet.is_running() {
            return Tick::Idle;
        }
        match self.sheet.step(&self.gpu) {
            Ok(None) => Tick::Working,
            Ok(Some(sheet)) => Tick::Done(sheet),
            Err(error) => Tick::Failed(error),
        }
    }
}

#[cfg(test)]
mod tests;
