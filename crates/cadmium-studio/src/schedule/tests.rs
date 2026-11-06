use cadmium_engine::stroke::StrokeSample;

use super::{FRAME_EVERY_SAMPLES, SETTLE_FRAMES, SETTLE_STEP_MS, StrokeSchedule, StudioEvent};
use crate::demo::{DemoStroke, StrokeBox, samples};

const AREA: StrokeBox = StrokeBox {
    x: 10.0,
    y: 20.0,
    width: 300.0,
    height: 100.0,
};

fn last_ms(stroke: DemoStroke) -> f64 {
    f64::from(samples(stroke, AREA).last().unwrap().time_ms)
}

/// Today's thumbnail driver, written out by hand.
fn expected_thumbnail(stroke: DemoStroke) -> Vec<StudioEvent> {
    let mut expected = Vec::new();
    for (index, sample) in samples(stroke, AREA).into_iter().enumerate() {
        let fed = match index {
            0 => StudioEvent::Begin(sample),
            _ => StudioEvent::Add(sample),
        };
        expected.push(fed);
        let shown = (index % FRAME_EVERY_SAMPLES == 0).then_some(sample.time_ms);
        expected.extend(shown.map(|t| StudioEvent::Frame(f64::from(t))));
    }
    expected.push(StudioEvent::End);
    let settle = (1..=SETTLE_FRAMES).map(|k| last_ms(stroke) + f64::from(k) * SETTLE_STEP_MS);
    expected.extend(settle.map(StudioEvent::Frame));
    expected
}

#[test]
fn the_thumbnail_schedule_feeds_samples_with_a_frame_every_few() {
    for stroke in [DemoStroke::SCurve, DemoStroke::PressureRamp] {
        let schedule = StrokeSchedule::thumbnail(stroke, AREA);
        let want = expected_thumbnail(stroke);
        assert_eq!(schedule.events(), want.as_slice(), "{stroke:?}");
        check_structure(schedule.events(), stroke);
    }
}

fn check_structure(events: &[StudioEvent], stroke: DemoStroke) {
    assert!(matches!(events[0], StudioEvent::Begin(_)), "{stroke:?}");
    let ends: Vec<usize> = (0..events.len())
        .filter(|&at| events[at] == StudioEvent::End)
        .collect();
    assert_eq!(ends.len(), 1, "{stroke:?}");
    let after = &events[ends[0] + 1..];
    let settle = after.iter().filter(|e| matches!(e, StudioEvent::Frame(_)));
    assert_eq!(settle.count(), SETTLE_FRAMES as usize, "{stroke:?}");
    assert_eq!(after.len(), SETTLE_FRAMES as usize, "{stroke:?}");
    let clocks: Vec<f64> = events
        .iter()
        .filter_map(|e| match e {
            StudioEvent::Frame(clock_ms) => Some(*clock_ms),
            _ => None,
        })
        .collect();
    assert!(clocks.windows(2).all(|w| w[0] <= w[1]), "{stroke:?}");
}

/// `event`, `by_ms` later, written out by hand.
fn later(event: StudioEvent, by_ms: u32) -> StudioEvent {
    let moved = |sample: StrokeSample| StrokeSample {
        time_ms: sample.time_ms + by_ms,
        ..sample
    };
    match event {
        StudioEvent::Begin(sample) => StudioEvent::Begin(moved(sample)),
        StudioEvent::Add(sample) => StudioEvent::Add(moved(sample)),
        StudioEvent::Frame(clock_ms) => StudioEvent::Frame(clock_ms + f64::from(by_ms)),
        StudioEvent::End => StudioEvent::End,
    }
}

#[test]
fn a_shifted_schedule_moves_every_clock_forward_together() {
    let schedule = StrokeSchedule::thumbnail(DemoStroke::PressureRamp, AREA);
    let want: Vec<StudioEvent> = schedule.events().iter().map(|e| later(*e, 1000)).collect();
    assert_eq!(schedule.shifted(1000).events(), want.as_slice());
}

#[test]
fn the_last_clock_is_where_the_settle_ends() {
    let schedule = StrokeSchedule::thumbnail(DemoStroke::SCurve, AREA);
    let want = last_ms(DemoStroke::SCurve) + f64::from(SETTLE_FRAMES) * SETTLE_STEP_MS;
    assert_eq!(schedule.last_clock_ms(), want);
    assert_eq!(schedule.shifted(250).last_clock_ms(), want + 250.0);
}
