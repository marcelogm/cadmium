//! When the studio feeds each pen sample and shows each frame, as plain data.

use cadmium_engine::engine::BRISTLE_SETTLE_MS;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::demo::{DemoStroke, StrokeBox, samples};

/// The animated clock's step: one frame per demo sample interval.
pub const QUANTUM_MS: f64 = 6.0;
/// How long the animated stroke keeps showing frames after the lift.
pub const SETTLE_MS: f64 = BRISTLE_SETTLE_MS as f64;
const FRAME_EVERY_SAMPLES: usize = 3;
const SETTLE_FRAMES: u32 = 20;
const SETTLE_STEP_MS: f64 = 60.0;

/// One step of a studio stroke.
#[derive(Clone, Copy, Debug)]
pub enum StudioEvent {
    Begin(StrokeSample),
    Add(StrokeSample),
    /// Shows a frame at this clock, in milliseconds.
    Frame(f64),
    End,
}

/// A demo stroke as the ordered events that paint it.
#[derive(Clone, Debug)]
pub struct StrokeSchedule {
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

/// Feeds every sample from `fed` on that is due by `clock_ms`, and the lift after
/// the last; returns how many samples are fed in all.
fn feed_due(
    strokes: &[StrokeSample],
    fed: usize,
    clock_ms: f64,
    events: &mut Vec<StudioEvent>,
) -> usize {
    let due = strokes[fed..]
        .iter()
        .take_while(|s| f64::from(s.time_ms) <= clock_ms)
        .count();
    let fresh = strokes[fed..fed + due].iter().copied().enumerate();
    events.extend(fresh.map(|(offset, sample)| feed(fed + offset, sample)));
    let now = fed + due;
    if due > 0 && now == strokes.len() {
        events.push(StudioEvent::End);
    }
    now
}

fn last_time_ms(strokes: &[StrokeSample]) -> f64 {
    strokes.last().map_or(0.0, |s| f64::from(s.time_ms))
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

    /// Real time: a frame every quantum from 0 until the bristles have settled.
    pub fn animated(stroke: DemoStroke, area: StrokeBox) -> StrokeSchedule {
        let strokes = samples(stroke, area);
        let settled_ms = last_time_ms(&strokes) + SETTLE_MS;
        let quanta = (settled_ms / QUANTUM_MS).ceil() as u32;
        let mut events = Vec::new();
        let mut fed = 0;
        for quantum in 0..=quanta {
            let clock_ms = f64::from(quantum) * QUANTUM_MS;
            fed = feed_due(&strokes, fed, clock_ms, &mut events);
            events.push(StudioEvent::Frame(clock_ms));
        }
        StrokeSchedule { events }
    }

    pub fn events(&self) -> &[StudioEvent] {
        &self.events
    }

    /// The clock of the last frame.
    pub fn duration_ms(&self) -> f64 {
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
