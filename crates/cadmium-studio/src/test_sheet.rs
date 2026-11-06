//! The editor's test sheet: a canvas of ruled paper the painter tries the draft brush on by
//! hand, with the painting's paint, in a private session of its own (no undo).
//!
//! A new brush waits for the next stroke, and every stroke begins with a fresh dip, so each
//! stroke shows the brush itself rather than a reservoir the strokes before ran down.

use cadmium_engine::brush::BrushSpec;
use cadmium_engine::canvas::{CanvasSpec, Primer, Weave};
use cadmium_engine::engine::PaintEngine;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::present::{PresentRequest, ViewTransform};
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::example::replace_canvas;
use crate::loading::{Loading, wear};
use crate::paint::StudioPaint;
use crate::studio::StudioError;

mod ground;

/// The editor's test sheet canvas, in canvas px: 4:3, filling whatever room the editor
/// gives it (the overflow cropped).
pub const TEST_SHEET_SIZE: (u32, u32) = (1200, 900);
/// The sheet's ground: plain paper, whatever the painting is primed with, ruled with a grid.
const GROUND: (Primer, Weave) = (Primer::Paper, Weave::Smooth);

/// One frame of a test sheet: the clock it is shown at, and where its canvas sits in the
/// target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetFrame {
    pub clock_ms: f64,
    pub view: ViewTransform,
}

/// A canvas to try a brush on by hand. Its paint is fixed when it is made: a new paint
/// takes a new sheet.
pub struct TestSheet {
    session: PaintSession,
    paint: StudioPaint,
    /// The brush the next stroke begins with.
    brush: BrushSpec,
}

impl TestSheet {
    /// A blank `size` canvas of plain paper with `paint`'s slots, under `paint`'s simulation
    /// settings; its strokes take `brush`, loaded with `paint`. The painting's primer and
    /// weave are not used: the sheet reads as the editor's paper.
    pub fn new(
        gpu: &GpuContext,
        paint: &StudioPaint,
        brush: &BrushSpec,
        size: (u32, u32),
    ) -> Result<TestSheet, StudioError> {
        let spec = CanvasSpec {
            width: size.0,
            height: size.1,
            primer: GROUND.0,
            weave: GROUND.1,
            pigment_slots: paint.slots.clone(),
        };
        let mut session = PaintSession::headless(gpu, spec)?;
        ground::rule(&mut session.canvas, gpu, &session.db);
        *session.engine.params_mut() = paint.params.clone();
        Ok(TestSheet {
            session,
            paint: paint.clone(),
            brush: brush.clone(),
        })
    }

    /// The canvas size, in canvas px.
    pub fn size(&self) -> (u32, u32) {
        let spec = &self.session.canvas.spec;
        (spec.width, spec.height)
    }

    /// The brush the next stroke takes. A stroke under way keeps its own, and what is
    /// painted stays.
    pub fn set_brush(&mut self, brush: &BrushSpec) {
        self.brush = brush.clone();
    }

    /// Ends a stroke still under way, puts the brush in hand, freshly loaded with the paint,
    /// then starts a stroke at `sample`.
    pub fn begin_stroke(&mut self, gpu: &GpuContext, sample: StrokeSample) {
        self.session.end_stroke();
        let loading = Loading {
            recipe: &self.paint.recipe,
            load: self.paint.load,
            medium: self.paint.medium,
        };
        wear(&mut self.session, gpu, &self.brush, loading);
        self.session.begin_stroke(gpu, sample);
    }

    /// Carries the stroke under way on through `samples`; without one, does nothing.
    pub fn add_samples(&mut self, samples: &[StrokeSample]) {
        self.session.add_samples(samples);
    }

    /// Lifts the brush off the sheet; what is left of the stroke paints on later frames.
    pub fn end_stroke(&mut self) {
        self.session.end_stroke();
    }

    /// A blank, ruled canvas again; the brush for the next stroke is kept. A failed swap may
    /// leave a 1×1 canvas behind: the sheet is then of no further use.
    pub fn clear(&mut self, gpu: &GpuContext) -> Result<(), StudioError> {
        self.session.end_stroke();
        self.session.flush(gpu);
        let spec = self.session.canvas.spec.clone();
        replace_canvas(&mut self.session, gpu, spec)?;
        ground::rule(&mut self.session.canvas, gpu, &self.session.db);
        Ok(())
    }

    /// Whether frames at `clock_ms` still change the sheet: dabs or tiles to paint, or
    /// bristles settling after a stroke.
    pub fn needs_frames(&self, clock_ms: f64) -> bool {
        self.session.has_pending_work() || self.session.bristles_settling(clock_ms)
    }

    /// Paints what is due at the frame's clock, then presents the canvas into `target` as
    /// the frame's view frames it.
    pub fn encode_present(
        &mut self,
        gpu: &GpuContext,
        frame: SheetFrame,
        target: &wgpu::Texture,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.session.encode_frame(gpu, frame.clock_ms, encoder);
        let request = PresentRequest::of_canvas(&self.session.canvas, frame.view);
        self.session
            .presenter
            .encode_present(gpu, request, target, encoder);
    }
}

#[cfg(test)]
mod tests;
