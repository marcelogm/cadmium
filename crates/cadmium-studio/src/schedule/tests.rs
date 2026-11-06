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
    for stroke in [DemoStroke::SCurve, DemoStroke::RisingLine] {
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
