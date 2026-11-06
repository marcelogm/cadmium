//! A brush's demo stroke played back in real time, in a loop.

use cadmium_engine::brush::BrushSpec;
use cadmium_engine::canvas::{Canvas, CanvasError, CanvasSpec, Primer, Weave};
use cadmium_engine::engine::PaintEngine;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::params::SimParams;
use cadmium_engine::present::{PresentRequest, ViewTransform, readback_rgba8};
use cadmium_engine::session::PaintSession;

use crate::demo::{DemoStroke, StrokeBox};
use crate::fit::StudioFit;
use crate::schedule::{QUANTUM_MS, StrokeSchedule, StudioEvent, replay};
use crate::studio::{Loading, StudioError, wear};

/// How long the finished stroke stays on show before the next loop.
pub const LOOP_PAUSE_MS: f64 = 700.0;
/// The slow-motion replay speed.
pub const SLOW_MOTION: f64 = 0.25;
/// The most quanta one `advance` replays; a longer stall drops the rest.
const MAX_QUANTA_PER_ADVANCE: u32 = 32;

/// What the studio canvas is made of and what the brush is loaded with.
#[derive(Clone, Debug, PartialEq)]
pub struct StudioPaint {
    pub slots: Vec<String>,
    pub primer: Primer,
    pub weave: Weave,
    pub recipe: Vec<(usize, f32)>,
    pub load: f32,
    pub medium: f32,
    pub params: SimParams,
}

/// Where a loop is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The pen is down, or about to be.
    Stroke,
    /// The pen is up and the bristles are settling.
    Settle,
    /// The finished stroke is on show; nothing is simulated.
    Pause,
}

/// What a player shows.
#[derive(Clone, Copy, Debug)]
pub struct PlayerRequest<'a> {
    pub brush: &'a BrushSpec,
    pub paint: &'a StudioPaint,
    pub stroke: DemoStroke,
}

/// How many quanta are due after `real_dt_ms` more at `speed`, and the time left
/// over. Past `cap` quanta the excess is dropped, keeping less than a quantum.
pub(crate) fn quanta_due(
    accumulated_ms: f64,
    real_dt_ms: f64,
    speed: f64,
    quantum_ms: f64,
    cap: u32,
) -> (u32, f64) {
    let total_ms = accumulated_ms + real_dt_ms.max(0.0) * speed;
    let due = (total_ms / quantum_ms).floor();
    if due > f64::from(cap) {
        return (cap, total_ms.rem_euclid(quantum_ms));
    }
    (due as u32, total_ms - due * quantum_ms)
}

/// Everything a request decides.
struct Show {
    fit: StudioFit,
    paint: StudioPaint,
    stroke: DemoStroke,
    schedule: StrokeSchedule,
    end_at: usize,
}

impl Show {
    fn of(request: PlayerRequest<'_>) -> Show {
        let fit = StudioFit::of(request.brush);
        let schedule = StrokeSchedule::animated(request.stroke, fit.stroke_box());
        let events = schedule.events();
        let end_at = events.iter().position(|e| *e == StudioEvent::End);
        Show {
            end_at: end_at.unwrap_or(events.len()),
            fit,
            paint: request.paint.clone(),
            stroke: request.stroke,
            schedule,
        }
    }

    fn canvas_spec(&self) -> CanvasSpec {
        CanvasSpec {
            width: self.fit.canvas.0,
            height: self.fit.canvas.1,
            primer: self.paint.primer,
            weave: self.paint.weave,
            pigment_slots: self.paint.slots.clone(),
        }
    }

    /// The simulation settings and a freshly loaded brush.
    fn dress(&self, session: &mut PaintSession, gpu: &GpuContext) {
        *session.engine.params_mut() = self.paint.params.clone();
        let loading = Loading {
            recipe: &self.paint.recipe,
            load: self.paint.load,
            medium: self.paint.medium,
        };
        wear(session, gpu, &self.fit.brush, loading);
    }

    /// One loop: the stroke and its settle, one quantum per frame, then the pause.
    fn loop_ms(&self) -> f64 {
        self.schedule.duration_ms() + QUANTUM_MS + LOOP_PAUSE_MS
    }
}

/// Drops the old canvas before allocating the new one, so two never coexist.
fn replace_canvas(
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

/// Plays a brush's demo stroke in a loop, on its own canvas.
///
/// The simulation always steps one quantum at a time at the same clocks; speed
/// and pause only change how much real time a quantum takes, so every speed
/// paints the same pixels. Each loop starts on a fresh canvas with a freshly
/// loaded brush and the clock at 0.
pub struct StudioPlayer {
    session: PaintSession,
    show: Show,
    /// How many events this loop has played.
    cursor: usize,
    /// How many quanta this loop has run.
    quanta: u32,
    accumulated_ms: f64,
    speed: f64,
    playing: bool,
}

impl StudioPlayer {
    pub fn new(gpu: &GpuContext, request: PlayerRequest<'_>) -> Result<StudioPlayer, StudioError> {
        let show = Show::of(request);
        let mut session = PaintSession::headless(gpu, show.canvas_spec())?;
        show.dress(&mut session, gpu);
        Ok(StudioPlayer {
            session,
            show,
            cursor: 0,
            quanta: 0,
            accumulated_ms: 0.0,
            speed: 1.0,
            playing: true,
        })
    }

    /// Replays the quanta due after `real_dt_ms` of real time.
    pub fn advance(&mut self, gpu: &GpuContext, real_dt_ms: f64) -> Result<(), StudioError> {
        let speed = if self.playing { self.speed } else { 0.0 };
        let (due, rest_ms) = quanta_due(
            self.accumulated_ms,
            real_dt_ms,
            speed,
            QUANTUM_MS,
            MAX_QUANTA_PER_ADVANCE,
        );
        self.accumulated_ms = rest_ms;
        (0..due).try_for_each(|_| self.step(gpu))
    }

    /// Shows `request` from the start of a loop, keeping the speed and pause.
    ///
    /// On an error the player waits at the end of a loop and tries the canvas
    /// again when it next advances.
    pub fn restart(
        &mut self,
        gpu: &GpuContext,
        request: PlayerRequest<'_>,
    ) -> Result<(), StudioError> {
        self.show = Show::of(request);
        self.accumulated_ms = 0.0;
        self.finish_loop();
        self.start_loop(gpu)
    }

    /// Replays, ignoring speed and pause, until the loop clock reaches
    /// `loop_ms` or the loop's end.
    pub fn run_until(&mut self, gpu: &GpuContext, loop_ms: f64) -> Result<(), StudioError> {
        let target_ms = loop_ms.min(self.loop_ms());
        while self.loop_clock_ms() < target_ms {
            self.step(gpu)?;
        }
        Ok(())
    }

    /// Negative speeds stop the replay like a pause.
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.max(0.0);
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn phase(&self) -> Phase {
        let Some(shown) = self.quanta.checked_sub(1) else {
            return Phase::Stroke;
        };
        if f64::from(shown) * QUANTUM_MS > self.show.schedule.duration_ms() {
            Phase::Pause
        } else if self.cursor > self.show.end_at {
            Phase::Settle
        } else {
            Phase::Stroke
        }
    }

    /// How long one loop lasts at full speed, pause included.
    pub fn loop_ms(&self) -> f64 {
        self.show.loop_ms()
    }

    /// The brush as painted: the requested one, scaled down if it did not fit.
    pub fn brush(&self) -> &BrushSpec {
        &self.show.fit.brush
    }

    pub fn paint(&self) -> &StudioPaint {
        &self.show.paint
    }

    pub fn stroke(&self) -> DemoStroke {
        self.show.stroke
    }

    pub fn session(&self) -> &PaintSession {
        &self.session
    }

    /// Where on the canvas the stroke runs.
    pub fn stroke_box(&self) -> StrokeBox {
        self.show.fit.stroke_box()
    }

    /// How far the painted brush's hairs reach from the ferrule centre, in px.
    pub fn reach_px(&self) -> f32 {
        self.show.fit.reach_px
    }

    /// Whether the brush is on show: from the loop's first frame until the pause.
    pub fn brush_visible(&self) -> bool {
        self.quanta > 0 && self.phase() != Phase::Pause
    }

    /// The canvas as shown, at 1:1, as straight sRGB RGBA8.
    pub fn rgba(&self, gpu: &GpuContext) -> Result<Vec<u8>, StudioError> {
        let canvas = &self.session.canvas;
        let presenter = &self.session.presenter;
        let target = presenter.create_target(gpu, canvas.spec.width, canvas.spec.height);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let view = ViewTransform {
            origin_x: 0.0,
            origin_y: 0.0,
            zoom: 1.0,
        };
        let request = PresentRequest::of_canvas(canvas, view);
        presenter.encode_present(gpu, request, &target, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        Ok(readback_rgba8(gpu, &target)?)
    }

    fn loop_clock_ms(&self) -> f64 {
        f64::from(self.quanta) * QUANTUM_MS
    }

    /// One quantum: the next loop if this one is over, then this quantum's frame.
    fn step(&mut self, gpu: &GpuContext) -> Result<(), StudioError> {
        if self.loop_clock_ms() >= self.loop_ms() {
            self.start_loop(gpu)?;
        }
        let cut = self.show.schedule.events_until(self.loop_clock_ms());
        replay(
            &mut self.session,
            gpu,
            &self.show.schedule.events()[self.cursor..cut],
        );
        self.cursor = cut;
        self.quanta += 1;
        Ok(())
    }

    fn start_loop(&mut self, gpu: &GpuContext) -> Result<(), StudioError> {
        replace_canvas(&mut self.session, gpu, self.show.canvas_spec())?;
        self.show.dress(&mut self.session, gpu);
        self.cursor = 0;
        self.quanta = 0;
        Ok(())
    }

    /// Everything played and the pause over: the next quantum starts a loop.
    fn finish_loop(&mut self) {
        self.cursor = self.show.schedule.events().len();
        self.quanta = (self.loop_ms() / QUANTUM_MS).ceil() as u32;
    }
}

#[cfg(test)]
mod tests;
