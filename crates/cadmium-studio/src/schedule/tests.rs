use super::{
    FRAME_EVERY_SAMPLES, QUANTUM_MS, SETTLE_FRAMES, SETTLE_MS, SETTLE_STEP_MS, StrokeSchedule,
    StudioEvent,
};
use crate::demo::{DemoStroke, StrokeBox, samples};
use cadmium_engine::stroke::StrokeSample;

const AREA: StrokeBox = StrokeBox {
    x: 10.0,
    y: 20.0,
    width: 300.0,
    height: 100.0,
};

fn frames(events: &[StudioEvent]) -> Vec<f64> {
    events
        .iter()
        .filter_map(|e| match e {
            StudioEvent::Frame(clock_ms) => Some(*clock_ms),
            _ => None,
        })
        .collect()
}

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
    for stroke in DemoStroke::ALL {
        let schedule = StrokeSchedule::thumbnail(stroke, AREA);
        let want = expected_thumbnail(stroke);
        assert_eq!(schedule.events(), want.as_slice(), "{stroke:?}");
        let settled = last_ms(stroke) + f64::from(SETTLE_FRAMES) * SETTLE_STEP_MS;
        assert_eq!(schedule.duration_ms(), settled, "{stroke:?}");
    }
}

#[test]
fn the_animated_schedule_shows_one_frame_per_quantum() {
    for stroke in DemoStroke::ALL {
        let clocks = frames(StrokeSchedule::animated(stroke, AREA).events());
        let want: Vec<f64> = (0..clocks.len()).map(|k| k as f64 * QUANTUM_MS).collect();
        assert_eq!(clocks, want, "{stroke:?}");
    }
}

/// Each fed sample with the clocks of the frames just before and just after it.
fn fed_between_frames(events: &[StudioEvent]) -> Vec<(StrokeSample, Option<f64>, f64)> {
    let mut fed = Vec::new();
    let mut previous = None;
    for (at, event) in events.iter().enumerate() {
        let next = || frames(&events[at..])[0];
        match event {
            StudioEvent::Begin(s) | StudioEvent::Add(s) => fed.push((*s, previous, next())),
            StudioEvent::Frame(clock_ms) => previous = Some(*clock_ms),
            StudioEvent::End => {}
        }
    }
    fed
}

#[test]
fn the_animated_schedule_feeds_each_sample_before_its_frame() {
    for stroke in DemoStroke::ALL {
        let schedule = StrokeSchedule::animated(stroke, AREA);
        assert!(matches!(schedule.events()[0], StudioEvent::Begin(_)));
        let fed = fed_between_frames(schedule.events());
        for (sample, previous, next) in &fed {
            let due = f64::from(sample.time_ms);
            assert!(previous.is_none_or(|c| c < due), "{stroke:?} {due}");
            assert!(*next >= due, "{stroke:?} {due}");
        }
        let fed: Vec<StrokeSample> = fed.into_iter().map(|(s, _, _)| s).collect();
        assert_eq!(fed, samples(stroke, AREA), "{stroke:?}");
    }
}

#[test]
fn the_animated_schedule_ends_after_the_last_sample_then_settles() {
    for stroke in DemoStroke::ALL {
        let schedule = StrokeSchedule::animated(stroke, AREA);
        let events = schedule.events();
        let end = events
            .iter()
            .position(|e| matches!(e, StudioEvent::End))
            .unwrap();
        assert!(matches!(events[end - 1], StudioEvent::Add(_)), "{stroke:?}");
        let after = &events[end + 1..];
        assert!(after.iter().all(|e| matches!(e, StudioEvent::Frame(_))));
        let settled_ms = last_ms(stroke) + SETTLE_MS;
        let duration = schedule.duration_ms();
        assert!(duration >= settled_ms && duration < settled_ms + QUANTUM_MS);
        assert_eq!(duration, *frames(events).last().unwrap(), "{stroke:?}");
    }
}

#[test]
fn a_cut_at_a_clock_falls_just_after_the_last_frame_shown_by_then() {
    let schedule = StrokeSchedule::animated(DemoStroke::SCurve, AREA);
    let events = schedule.events();
    assert_eq!(schedule.events_until(-1.0), 0);
    for clock_ms in [0.0, 5.9, 6.0, 100.0, 1e9] {
        let cut = schedule.events_until(clock_ms);
        let shown = frames(&events[..cut]);
        assert_eq!(
            shown.last(),
            frames(events).iter().rfind(|c| **c <= clock_ms)
        );
        assert!(
            matches!(events[cut - 1], StudioEvent::Frame(_)),
            "{clock_ms}"
        );
    }
    assert_eq!(schedule.events_until(5.9), schedule.events_until(0.0));
    assert_eq!(schedule.events_until(1e9), events.len());
}

#[test]
fn a_cut_works_on_the_thumbnail_schedule_too() {
    let schedule = StrokeSchedule::thumbnail(DemoStroke::TapAndLift, AREA);
    let events = schedule.events();
    assert_eq!(schedule.events_until(0.0), 2);
    assert_eq!(schedule.events_until(schedule.duration_ms()), events.len());
    let before_settle = schedule.events_until(schedule.duration_ms() - 1.0);
    assert_eq!(before_settle, events.len() - 1);
}
