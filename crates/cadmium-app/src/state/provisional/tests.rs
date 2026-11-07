use super::*;

const UPRIGHT: ViewTransform = ViewTransform {
    origin_x: 10.0,
    origin_y: 20.0,
    zoom: 2.0,
    rotation: 0.0,
};
const BLUE: [u8; 3] = [20, 40, 160];

fn dab_at(x: f32, radius: f32) -> Dab {
    Dab {
        x,
        y: 50.0,
        radius,
        pressure: 1.0,
        altitude: 0.0,
        azimuth: 0.0,
        direction: 0.0,
        speed: 0.0,
        time_ms: 0.0,
    }
}

fn row(count: usize, step_px: f32, radius: f32) -> Vec<Dab> {
    (0..count)
        .map(|index| dab_at(index as f32 * step_px, radius))
        .collect()
}

#[test]
fn dabs_closer_than_half_a_radius_collapse_into_fewer_marks() {
    let trail = ProvisionalTrail::of(BLUE, &row(100, 1.0, 20.0), UPRIGHT);
    let marks = trail.canvas_marks();
    assert!(marks.len() <= 14, "{} marks", marks.len());
    assert!(
        marks
            .windows(2)
            .all(|pair| pair[1].centre.0 - pair[0].centre.0 <= 10.0)
    );
}

#[test]
fn the_tip_of_the_queue_is_always_kept() {
    let trail = ProvisionalTrail::of(BLUE, &row(7, 1.0, 20.0), UPRIGHT);
    let tip = trail.canvas_marks().last().copied();
    assert_eq!(tip.map(|mark| mark.centre.0), Some(6.0));
}

#[test]
fn well_spread_dabs_each_keep_their_mark() {
    let trail = ProvisionalTrail::of(BLUE, &row(5, 30.0, 20.0), UPRIGHT);
    assert_eq!(trail.canvas_marks().len(), 5);
}

#[test]
fn widget_marks_follow_the_view_the_texture_was_drawn_with() {
    let trail = ProvisionalTrail::of(BLUE, &[dab_at(30.0, 5.0)], UPRIGHT);
    let marks = trail.widget_marks(2.0);
    assert_eq!(
        marks,
        vec![Mark {
            centre: (20.0, 30.0),
            radius: 5.0
        }]
    );
}

#[test]
fn no_queued_dabs_leave_no_marks() {
    assert!(
        ProvisionalTrail::of(BLUE, &[], UPRIGHT)
            .canvas_marks()
            .is_empty()
    );
}

#[test]
fn painting_the_head_of_the_queue_leaves_the_marks_behind_it_in_place() {
    let queued = row(100, 1.0, 20.0);
    let before = ProvisionalTrail::of(BLUE, &queued, UPRIGHT);
    let after = ProvisionalTrail::of(BLUE, &queued[3..], UPRIGHT);
    for mark in &after.canvas_marks()[1..] {
        assert!(
            before.canvas_marks().contains(mark),
            "the mark at {:?} moved",
            mark.centre
        );
    }
}

const FRAME_MS: f64 = 16.0;

fn fade_after(frames: &[bool]) -> f32 {
    let mut fade = TrailFade::default();
    let mut opacity = 0.0;
    for (index, &showing) in frames.iter().enumerate() {
        opacity = fade.advance(index as f64 * FRAME_MS, showing);
    }
    opacity
}

#[test]
fn a_short_lag_shows_no_trail() {
    assert_eq!(fade_after(&[false, true, true, true]), 0.0);
}

#[test]
fn a_lag_that_lasts_brings_the_trail_in_faint() {
    let frames = (TRAIL_DELAY_MS / FRAME_MS).ceil() as usize + 1;
    let mut showing = vec![true; frames];
    showing.insert(0, false);
    let opacity = fade_after(&showing);
    assert!(opacity > 0.0, "{opacity}");
    assert!(opacity < PROVISIONAL_OPACITY / 4.0, "{opacity}");
}

#[test]
fn a_lag_that_stays_brings_the_trail_to_full_strength() {
    let frames = ((TRAIL_DELAY_MS + TRAIL_FADE_MS) / FRAME_MS).ceil() as usize;
    let mut showing = vec![true; frames];
    showing.insert(0, false);
    assert_eq!(fade_after(&showing), PROVISIONAL_OPACITY);
}

const REFRESH_MS: f64 = 1000.0 / 60.0;
const JITTER_MS: f64 = 0.5;

/// Every frame's opacity over 120 frames at 60 Hz with the gaps alternating a little long
/// and a little short.
fn jittery_opacities(showing: impl Fn(usize) -> bool) -> Vec<f32> {
    let mut fade = TrailFade::default();
    let mut now_ms = 0.0;
    (0..120)
        .map(|frame| {
            now_ms += if frame % 2 == 1 {
                REFRESH_MS + JITTER_MS
            } else {
                REFRESH_MS - JITTER_MS
            };
            fade.advance(now_ms, showing(frame))
        })
        .collect()
}

#[test]
fn a_lag_that_comes_and_goes_every_frame_shows_no_trail() {
    for (frame, opacity) in jittery_opacities(|frame| frame % 2 == 1)
        .into_iter()
        .enumerate()
    {
        assert_eq!(opacity, 0.0, "frame {frame}");
    }
}

#[test]
fn a_lag_on_three_frames_in_four_shows_no_trail() {
    for (frame, opacity) in jittery_opacities(|frame| frame % 4 != 0)
        .into_iter()
        .enumerate()
    {
        assert_eq!(opacity, 0.0, "frame {frame}");
    }
}

#[test]
fn a_lag_on_three_frames_in_four_stays_hidden_when_lag_frames_run_long() {
    let mut fade = TrailFade::default();
    let mut now_ms = 0.0;
    for frame in 0..600 {
        let showing = frame % 4 != 0;
        now_ms += if showing {
            REFRESH_MS + 1.0
        } else {
            REFRESH_MS - 1.0
        };
        assert_eq!(fade.advance(now_ms, showing), 0.0, "frame {frame}");
    }
}

/// The opacity after a lag of `lag_ms`, built in four equal steps.
fn opacity_after_lag(lag_ms: f64) -> f32 {
    let mut fade = TrailFade::default();
    fade.advance(0.0, false);
    let mut opacity = 0.0;
    for step in 1..=4 {
        opacity = fade.advance(f64::from(step) * lag_ms / 4.0, true);
    }
    opacity
}

#[test]
fn a_lag_just_under_the_delay_shows_no_trail() {
    assert_eq!(opacity_after_lag(TRAIL_DELAY_MS - 0.1), 0.0);
}

#[test]
fn a_lag_just_over_the_delay_shows_a_trail() {
    assert!(opacity_after_lag(TRAIL_DELAY_MS + 0.1) > 0.0);
}

#[test]
fn a_trail_after_an_idle_gap_shows_nothing_at_first() {
    let mut fade = TrailFade::default();
    fade.advance(0.0, false);
    assert_eq!(fade.advance(5000.0, true), 0.0);
}

fn fade_at_full_strength() -> (TrailFade, usize) {
    let frames = ((TRAIL_DELAY_MS + TRAIL_FADE_MS) / FRAME_MS).ceil() as usize;
    let mut fade = TrailFade::default();
    fade.advance(0.0, false);
    for frame in 1..=frames {
        fade.advance(frame as f64 * FRAME_MS, true);
    }
    (fade, frames)
}

const FOUR_TIMES_FASTER: f64 = 4.0;

#[test]
fn a_pause_without_a_trail_hides_it_again() {
    let (mut fade, full_frames) = fade_at_full_strength();
    let drain_frames =
        ((TRAIL_DELAY_MS + TRAIL_FADE_MS) / (FOUR_TIMES_FASTER * FRAME_MS)).ceil() as usize;
    for frame in 1..=drain_frames {
        fade.advance((full_frames + frame) as f64 * FRAME_MS, false);
    }
    let next_ms = (full_frames + drain_frames + 1) as f64 * FRAME_MS;
    assert_eq!(fade.advance(next_ms, true), 0.0);
}

#[test]
fn a_clock_that_steps_back_does_not_fade_the_trail() {
    let (mut fade, _) = fade_at_full_strength();
    assert_eq!(fade.advance(0.0, false), PROVISIONAL_OPACITY);
}
