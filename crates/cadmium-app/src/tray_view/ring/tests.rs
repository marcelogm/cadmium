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
