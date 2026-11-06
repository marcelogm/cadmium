use std::time::Duration;

use super::*;

const OUTLINE: f32 = 12.0;
const TOLERANCE: f32 = 1e-5;

fn ring(level: f32, pressure: f32) -> LoadRing {
    LoadRing {
        level,
        pressure,
        colour: [180, 70, 40],
    }
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < TOLERANCE,
        "{actual} is not {expected}"
    );
}

#[test]
fn an_empty_brush_rings_wide_and_faint() {
    let shape = ring_shape(OUTLINE, &ring(0.0, 0.5));
    assert_close(shape.radius, OUTLINE + 30.0);
    assert_close(shape.alpha, 0.35);
}

#[test]
fn a_full_brush_rings_on_its_outline() {
    let shape = ring_shape(OUTLINE, &ring(1.0, 0.5));
    assert_close(shape.radius, OUTLINE);
    assert_close(shape.alpha, 0.95);
}

#[test]
fn pressure_thickens_the_ring() {
    assert_close(ring_shape(OUTLINE, &ring(0.5, 0.0)).width, 2.0);
    assert_close(ring_shape(OUTLINE, &ring(0.5, 1.0)).width, 8.0);
}

#[test]
fn levels_outside_zero_to_one_are_clamped() {
    assert_eq!(
        ring_shape(OUTLINE, &ring(-0.5, -1.0)),
        ring_shape(OUTLINE, &ring(0.0, 0.0))
    );
    assert_eq!(
        ring_shape(OUTLINE, &ring(1.5, 2.0)),
        ring_shape(OUTLINE, &ring(1.0, 1.0))
    );
}

#[test]
fn a_released_ring_fades_where_it_was() {
    let shape = ring_shape(OUTLINE, &ring(0.5, 0.5));
    let half_way = fading_ring(shape, RING_FADE_SECONDS / 2.0).expect("still fading");
    assert_close(half_way.alpha, shape.alpha / 2.0);
    assert_eq!(half_way.radius, shape.radius);
    assert_eq!(half_way.width, shape.width);
}

#[test]
fn a_ring_is_gone_once_its_fade_ends() {
    let shape = ring_shape(OUTLINE, &ring(0.5, 0.5));
    assert_eq!(fading_ring(shape, RING_FADE_SECONDS), None);
}

const HOVER: (f64, f64) = (40.0, 50.0);
fn fade() -> Duration {
    Duration::from_secs_f32(RING_FADE_SECONDS)
}
const EDGE: Duration = Duration::from_millis(1);

fn shown_at(centre: (f64, f64), released_at: Option<Instant>) -> ShownRing {
    ShownRing {
        ring: ring(0.5, 0.5),
        centre,
        released_at,
    }
}

#[test]
fn a_live_ring_follows_the_pen() {
    let next = next_shown_ring(Some(ring(0.5, 0.5)), None, Some(HOVER), Instant::now()).unwrap();
    assert_eq!(next.centre, HOVER);
    assert!(next.released_at.is_none());
}

#[test]
fn a_live_ring_stays_put_when_the_pen_leaves() {
    let shown = shown_at((1.0, 2.0), None);
    let next = next_shown_ring(Some(ring(0.5, 0.5)), Some(shown), None, Instant::now()).unwrap();
    assert_eq!(next.centre, (1.0, 2.0));
}

#[test]
fn no_ring_and_no_pen_shows_nothing() {
    assert!(next_shown_ring(Some(ring(0.5, 0.5)), None, None, Instant::now()).is_none());
    assert!(next_shown_ring(None, None, Some(HOVER), Instant::now()).is_none());
}

#[test]
fn a_ring_that_ends_starts_fading_then_goes() {
    let start = Instant::now();
    let fading = next_shown_ring(None, Some(shown_at(HOVER, None)), None, start).unwrap();
    assert_eq!(fading.released_at, Some(start));
    let half = start + fade() / 2;
    assert!(next_shown_ring(None, Some(fading), None, half).is_some());
    assert!(next_shown_ring(None, Some(fading), None, start + fade() - EDGE).is_some());
    assert!(next_shown_ring(None, Some(fading), None, start + fade() + EDGE).is_none());
}

#[test]
fn a_fading_ring_is_drawn_fainter_until_it_is_gone() {
    let start = Instant::now();
    let live = visible_shape(&shown_at(HOVER, None), OUTLINE, start).unwrap();
    let fading = visible_shape(&shown_at(HOVER, Some(start)), OUTLINE, start + fade() / 2).unwrap();
    assert_close(fading.alpha, live.alpha / 2.0);
    let almost = start + fade() - EDGE;
    assert!(visible_shape(&shown_at(HOVER, Some(start)), OUTLINE, almost).is_some());
    let gone = start + fade() + EDGE;
    assert!(visible_shape(&shown_at(HOVER, Some(start)), OUTLINE, gone).is_none());
}

#[test]
fn a_live_ring_arriving_while_one_fades_restarts_live() {
    let start = Instant::now();
    let fading = shown_at(HOVER, Some(start));
    let next =
        next_shown_ring(Some(ring(0.5, 0.5)), Some(fading), None, start + fade() / 2).unwrap();
    assert!(next.released_at.is_none());
    assert_eq!(next.centre, HOVER);
}

#[test]
fn the_pens_hover_beats_where_the_ring_was_shown() {
    let shown = shown_at((1.0, 2.0), None);
    let next = next_shown_ring(
        Some(ring(0.5, 0.5)),
        Some(shown),
        Some(HOVER),
        Instant::now(),
    )
    .unwrap();
    assert_eq!(next.centre, HOVER);
}
