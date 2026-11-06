//! When the studio feeds each pen sample and shows each frame, as plain data.
//! The one cadence is the thumbnails' — a frame every few samples, then a fixed settle —
//! which the example sheet plays twice on one clock.

use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::demo::{DemoStroke, StrokeBox, samples};

const FRAME_EVERY_SAMPLES: usize = 3;
const SETTLE_FRAMES: u32 = 20;
const SETTLE_STEP_MS: f64 = 60.0;

/// One step of a studio stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum StudioEvent {
    Begin(StrokeSample),
    Add(StrokeSample),
    /// Shows a frame at this clock, in milliseconds.
    Frame(f64),
    End,
}

/// A demo stroke as the ordered events that paint it.
#[derive(Clone, Debug)]
pub(crate) struct StrokeSchedule {
    events: Vec<StudioEvent>,
}

fn feed(index: usize, sample: StrokeSample) -> StudioEvent {
    if index == 0 {
        StudioEvent::Begin(sample)
    } else {
        StudioEvent::Add(sample)
    }
}

/// The thumbnail cadence for one sample: fed, then shown every few samples.
fn thumbnail_touch((index, sample): (usize, StrokeSample)) -> impl Iterator<Item = StudioEvent> {
    let frame = index
        .is_multiple_of(FRAME_EVERY_SAMPLES)
        .then(|| StudioEvent::Frame(f64::from(sample.time_ms)));
    std::iter::once(feed(index, sample)).chain(frame)
}

fn last_time_ms(strokes: &[StrokeSample]) -> f64 {
    strokes.last().map_or(0.0, |s| f64::from(s.time_ms))
}

fn shift(event: StudioEvent, by_ms: u32) -> StudioEvent {
    let later = |sample: StrokeSample| StrokeSample {
        time_ms: sample.time_ms + by_ms,
        ..sample
    };
    match event {
        StudioEvent::Begin(sample) => StudioEvent::Begin(later(sample)),
        StudioEvent::Add(sample) => StudioEvent::Add(later(sample)),
        StudioEvent::Frame(clock_ms) => StudioEvent::Frame(clock_ms + f64::from(by_ms)),
        StudioEvent::End => StudioEvent::End,
    }
}

impl StrokeSchedule {
    /// The thumbnail cadence: a frame every few samples, then a fixed settle.
    pub fn thumbnail(stroke: DemoStroke, area: StrokeBox) -> StrokeSchedule {
        let strokes = samples(stroke, area);
        let mut events: Vec<StudioEvent> = strokes
            .iter()
            .copied()
            .enumerate()
            .flat_map(thumbnail_touch)
            .collect();
        events.push(StudioEvent::End);
        let mut clock_ms = last_time_ms(&strokes);
        for _ in 0..SETTLE_FRAMES {
            clock_ms += SETTLE_STEP_MS;
            events.push(StudioEvent::Frame(clock_ms));
        }
        StrokeSchedule { events }
    }

    /// Every event in the order it is played; frame clocks never decrease.
    pub fn events(&self) -> &[StudioEvent] {
        &self.events
    }

    /// The same events `by_ms` later: every sample and every frame clock moves together.
    pub fn shifted(&self, by_ms: u32) -> StrokeSchedule {
        let events = self
            .events
            .iter()
            .map(|event| shift(*event, by_ms))
            .collect();
        StrokeSchedule { events }
    }

    /// The clock of the last frame shown; 0 with no frame.
    pub fn last_clock_ms(&self) -> f64 {
        self.events
            .iter()
            .rev()
            .find_map(|event| match event {
                StudioEvent::Frame(clock_ms) => Some(*clock_ms),
                _ => None,
            })
            .unwrap_or(0.0)
    }
}

/// Plays `events` on `session`, submitting each frame on its own.
pub(crate) fn replay(session: &mut PaintSession, gpu: &GpuContext, events: &[StudioEvent]) {
    for event in events {
        match *event {
            StudioEvent::Begin(sample) => session.begin_stroke(gpu, sample),
            StudioEvent::Add(sample) => session.add_samples(&[sample]),
            StudioEvent::Frame(clock_ms) => {
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                session.encode_frame(gpu, clock_ms, &mut encoder);
                gpu.queue.submit([encoder.finish()]);
            }
            StudioEvent::End => session.end_stroke(),
        }
    }
}

#[cfg(test)]
mod tests;
