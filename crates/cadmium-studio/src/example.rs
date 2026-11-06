//! The editor's example strokes: an S-curve above a pressure ramp on one sheet,
//! painted by the real engine with a brush at its suggested size and the painting's paint.
//!
//! Every sheet starts on a fresh canvas with a freshly loaded brush and the clock at 0, so
//! the same brush and paint always paint the same pixels. The work comes in parts
//! (`begin`, then `step`), so the editor can spread it over idle ticks.

use cadmium_engine::brush::BrushSpec;
use cadmium_engine::canvas::{Canvas, CanvasError, CanvasSpec};
use cadmium_engine::engine::PaintEngine;
use cadmium_engine::gpu::{GpuContext, GpuLost};
use cadmium_engine::present::{PresentRequest, readback_rgba8};
use cadmium_engine::session::PaintSession;

use crate::demo::DemoStroke;
use crate::loading::{Loading, wear};
use crate::paint::StudioPaint;
use crate::schedule::{StrokeSchedule, replay};
use crate::studio::{StudioError, Thumbnail};

mod fit;
mod framing;

pub use fit::EXAMPLE_SHEET_SIZE;
use fit::ExampleFit;

/// The rest between the S-curve's settle and the ramp's first sample.
const RAMP_PAUSE_MS: u32 = 100;

/// What an example sheet shows: this brush, loaded with this paint.
#[derive(Clone, Copy, Debug)]
pub struct ExampleRequest<'a> {
    pub brush: &'a BrushSpec,
    pub paint: &'a StudioPaint,
}

/// The part of a sheet painted next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    SCurve,
    /// The ramp, from this clock, after a fresh dip.
    Ramp(u32),
    Present,
}

/// A sheet being painted.
struct Run {
    fit: ExampleFit,
    stage: Stage,
}

/// A private paint session that paints example sheets.
#[derive(Default)]
pub struct ExampleSheet {
    session: Option<PaintSession>,
    /// The canvas spec `session` was built for; `None` until a build succeeds.
    built_for: Option<CanvasSpec>,
    run: Option<Run>,
}

fn canvas_spec(fit: &ExampleFit, paint: &StudioPaint) -> CanvasSpec {
    CanvasSpec {
        width: fit.canvas.0,
        height: fit.canvas.1,
        primer: paint.primer,
        weave: paint.weave,
        pigment_slots: paint.slots.clone(),
    }
}

impl ExampleSheet {
    /// A sheet with no session yet: the first `begin` builds one.
    pub fn new() -> ExampleSheet {
        ExampleSheet::default()
    }

    /// Starts painting `request` on a fresh canvas, dropping any sheet under way.
    ///
    /// A new canvas spec (size, slots, primer or weave) gets a whole new session, so
    /// nothing of the old one lingers; otherwise only the canvas is replaced. The session
    /// never holds two full-size canvases.
    pub fn begin(
        &mut self,
        gpu: &GpuContext,
        request: ExampleRequest<'_>,
    ) -> Result<(), StudioError> {
        self.run = None;
        let fit = ExampleFit::of(request.brush);
        let session = self.fresh_session(gpu, canvas_spec(&fit, request.paint))?;
        dress(session, gpu, &fit.brush, request.paint);
        self.run = Some(Run {
            fit,
            stage: Stage::SCurve,
        });
        Ok(())
    }

    /// Paints the next part of the sheet under way — the S-curve, the ramp, then the
    /// presented sheet, which ends the run. `None` while parts remain, or with nothing begun.
    pub fn step(&mut self, gpu: &GpuContext) -> Result<Option<Thumbnail>, StudioError> {
        let (Some(session), Some(run)) = (self.session.as_mut(), self.run.as_mut()) else {
            return Ok(None);
        };
        let shown = advance(session, gpu, run);
        if gpu.is_lost() {
            self.run = None;
            return Err(GpuLost.into());
        }
        match shown {
            Ok(None) => Ok(None),
            done => {
                self.run = None;
                Ok(done?)
            }
        }
    }

    /// Paints a whole sheet at once.
    pub fn paint(
        &mut self,
        gpu: &GpuContext,
        request: ExampleRequest<'_>,
    ) -> Result<Thumbnail, StudioError> {
        self.begin(gpu, request)?;
        finish(self, gpu)
    }

    /// Whether a sheet is under way.
    pub fn is_running(&self) -> bool {
        self.run.is_some()
    }

    /// The session, on a fresh canvas of `spec`; the old session goes before a new one is made.
    fn fresh_session(
        &mut self,
        gpu: &GpuContext,
        spec: CanvasSpec,
    ) -> Result<&mut PaintSession, StudioError> {
        let reusable = self.built_for.as_ref() == Some(&spec);
        self.built_for = None;
        if reusable && let Some(session) = self.session.as_mut() {
            // A failed swap may leave the old canvas or a 1×1 one: drop the whole session.
            replace_canvas(session, gpu, spec.clone()).inspect_err(|_| self.session = None)?;
        } else {
            self.session = None;
            self.session = Some(PaintSession::headless(gpu, spec.clone())?);
        }
        self.built_for = Some(spec);
        Ok(self
            .session
            .as_mut()
            .expect("a session was just made ready"))
    }
}

/// Steps the sheet under way until it is presented.
fn finish(sheet: &mut ExampleSheet, gpu: &GpuContext) -> Result<Thumbnail, StudioError> {
    loop {
        if let Some(shown) = sheet.step(gpu)? {
            return Ok(shown);
        }
    }
}

/// Swaps the session's canvas for a fresh one of `spec`, through a 1×1 one, so two
/// full-size canvases never coexist.
pub(crate) fn replace_canvas(
    session: &mut PaintSession,
    gpu: &GpuContext,
    spec: CanvasSpec,
) -> Result<(), CanvasError> {
    let placeholder = CanvasSpec {
        width: 1,
        height: 1,
        ..spec.clone()
    };
    session.canvas = Canvas::new(gpu, &session.db, placeholder)?;
    session.canvas = Canvas::new(gpu, &session.db, spec)?;
    Ok(())
}

/// The painting's simulation settings, and the brush freshly loaded with its paint.
fn dress(session: &mut PaintSession, gpu: &GpuContext, brush: &BrushSpec, paint: &StudioPaint) {
    *session.engine.params_mut() = paint.params.clone();
    let loading = Loading {
        recipe: &paint.recipe,
        load: paint.load,
        medium: paint.medium,
    };
    wear(session, gpu, brush, loading);
}

fn advance(
    session: &mut PaintSession,
    gpu: &GpuContext,
    run: &mut Run,
) -> Result<Option<Thumbnail>, GpuLost> {
    match run.stage {
        Stage::SCurve => {
            run.stage = Stage::Ramp(paint_s_curve(session, gpu, &run.fit));
            Ok(None)
        }
        Stage::Ramp(from_ms) => {
            paint_ramp(session, gpu, &run.fit, from_ms);
            run.stage = Stage::Present;
            Ok(None)
        }
        Stage::Present => present(session, gpu, &run.fit).map(Some),
    }
}

/// Paints the S-curve from clock 0; returns the clock the ramp starts at.
fn paint_s_curve(session: &mut PaintSession, gpu: &GpuContext, fit: &ExampleFit) -> u32 {
    let schedule = StrokeSchedule::thumbnail(DemoStroke::SCurve, fit.s_curve_box());
    replay(session, gpu, schedule.events());
    schedule.last_clock_ms().ceil() as u32 + RAMP_PAUSE_MS
}

/// Loads the brush again (a fresh dip), then paints the ramp from `from_ms`.
fn paint_ramp(session: &mut PaintSession, gpu: &GpuContext, fit: &ExampleFit, from_ms: u32) {
    session.reload_brush(gpu);
    let schedule =
        StrokeSchedule::thumbnail(DemoStroke::PressureRamp, fit.ramp_box()).shifted(from_ms);
    replay(session, gpu, schedule.events());
}

/// The canvas fitted into the sheet; any band round it takes the canvas's bare paper.
fn present(
    session: &PaintSession,
    gpu: &GpuContext,
    fit: &ExampleFit,
) -> Result<Thumbnail, GpuLost> {
    let (width, height) = EXAMPLE_SHEET_SIZE;
    let view = framing::view_for(fit.canvas);
    let presenter = &session.presenter;
    let target = presenter.create_target(gpu, width, height);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let request = PresentRequest::of_canvas(&session.canvas, view);
    presenter.encode_present(gpu, request, &target, &mut encoder);
    gpu.queue.submit([encoder.finish()]);
    let rgba = readback_rgba8(gpu, &target)?;
    let mut sheet = Thumbnail {
        width,
        height,
        rgba,
    };
    let paper = framing::paper_colour(&sheet, view, fit.reach_px);
    framing::letterbox(&mut sheet, view, fit.canvas, paper);
    Ok(sheet)
}

#[cfg(test)]
mod tests;
