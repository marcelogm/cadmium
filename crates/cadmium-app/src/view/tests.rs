use std::f32::consts::{FRAC_PI_2, PI};

use super::*;

const TOLERANCE: f64 = 1e-3;

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < TOLERANCE && (a.1 - b.1).abs() < TOLERANCE
}

#[test]
fn fitted_canvas_is_centered_with_the_margin_on_the_tight_axis() {
    let view = fit_view((2000, 1000), (1048.0, 848.0), 24.0);
    assert!((view.zoom - 0.5).abs() < 1e-6);
    assert!(close(viewport_to_canvas(view, 24.0, 424.0), (0.0, 500.0)));
    assert!(close(
        viewport_to_canvas(view, 1024.0, 424.0),
        (2000.0, 500.0)
    ));
}

#[test]
fn zooming_keeps_the_canvas_point_under_the_pointer() {
    let view = fit_view((2048, 1536), (1200.0, 800.0), 24.0);
    let before = viewport_to_canvas(view, 300.0, 200.0);
    let zoomed = zoom_around(view, 300.0, 200.0, view.zoom * 3.0);
    assert!(close(viewport_to_canvas(zoomed, 300.0, 200.0), before));
}

#[test]
fn zoom_is_clamped_to_the_supported_range() {
    let view = fit_view((100, 100), (800.0, 800.0), 0.0);
    assert_eq!(zoom_around(view, 0.0, 0.0, 1000.0).zoom, MAX_ZOOM);
    assert_eq!(zoom_around(view, 0.0, 0.0, 0.0).zoom, MIN_ZOOM);
    assert_eq!(clamp_zoom(f32::NAN), 1.0);
}

#[test]
fn panning_moves_the_canvas_with_the_pointer() {
    let view = fit_view((1000, 1000), (500.0, 500.0), 0.0);
    let under_pointer = viewport_to_canvas(view, 100.0, 100.0);
    let panned = pan(view, 40.0, -20.0);
    assert!(close(
        viewport_to_canvas(panned, 140.0, 80.0),
        under_pointer
    ));
}

#[test]
fn tiny_viewport_still_produces_a_finite_view() {
    let view = fit_view((4096, 4096), (10.0, 10.0), 24.0);
    assert!(view.zoom >= MIN_ZOOM);
    assert!(view.origin_x.is_finite() && view.origin_y.is_finite());
}

fn inside(canvas: (u32, u32), (x, y): (f64, f64)) -> bool {
    let (width, height) = (f64::from(canvas.0), f64::from(canvas.1));
    (-TOLERANCE..=width + TOLERANCE).contains(&x) && (-TOLERANCE..=height + TOLERANCE).contains(&y)
}

#[test]
fn a_cover_view_fills_a_tall_viewport_cropping_the_sides() {
    let view = cover_view((1200, 900), (600, 900));
    assert_eq!(view.zoom, 1.0);
    assert!(close(
        viewport_to_canvas(view, 300.0, 450.0),
        (600.0, 450.0)
    ));
    assert!(close(viewport_to_canvas(view, 0.0, 0.0), (300.0, 0.0)));
    assert!(close(
        viewport_to_canvas(view, 600.0, 900.0),
        (900.0, 900.0)
    ));
}

#[test]
fn a_cover_view_fills_a_wide_viewport_cropping_top_and_bottom() {
    let view = cover_view((1200, 900), (1600, 900));
    assert!((view.zoom - 4.0 / 3.0).abs() < 1e-6);
    assert!(close(
        viewport_to_canvas(view, 800.0, 450.0),
        (600.0, 450.0)
    ));
    assert!(close(viewport_to_canvas(view, 0.0, 0.0), (0.0, 112.5)));
    assert!(close(
        viewport_to_canvas(view, 1600.0, 900.0),
        (1200.0, 787.5)
    ));
}

#[test]
fn every_viewport_corner_lands_on_the_canvas() {
    for viewport in [(432, 728), (728, 432), (1200, 900), (37, 1000)] {
        let view = cover_view((1200, 900), viewport);
        let (width, height) = (f64::from(viewport.0), f64::from(viewport.1));
        for (x, y) in [(0.0, 0.0), (width, 0.0), (0.0, height), (width, height)] {
            let point = viewport_to_canvas(view, x, y);
            assert!(inside((1200, 900), point), "{viewport:?} {point:?}");
        }
    }
}

fn turned(rotation: f32) -> ViewTransform {
    ViewTransform {
        origin_x: 120.0,
        origin_y: -40.0,
        zoom: 2.5,
        rotation,
    }
}

#[test]
fn screen_and_canvas_points_round_trip_at_any_turn() {
    for rotation in [0.0, 0.3, FRAC_PI_2, -2.0, PI] {
        let view = turned(rotation);
        let canvas = viewport_to_canvas(view, 310.0, 75.0);
        assert!(close(
            canvas_to_viewport(view, canvas.0, canvas.1),
            (310.0, 75.0)
        ));
    }
}

#[test]
fn a_quarter_turn_shows_the_canvas_x_axis_going_down_the_screen() {
    let view = ViewTransform {
        origin_x: 0.0,
        origin_y: 0.0,
        zoom: 1.0,
        rotation: FRAC_PI_2,
    };
    assert!(close(canvas_to_viewport(view, 10.0, 0.0), (0.0, 10.0)));
}

#[test]
fn zooming_a_turned_view_keeps_the_canvas_point_under_the_pointer() {
    let view = turned(0.7);
    let before = viewport_to_canvas(view, 300.0, 200.0);
    let zoomed = zoom_around(view, 300.0, 200.0, view.zoom * 3.0);
    assert!(close(viewport_to_canvas(zoomed, 300.0, 200.0), before));
    assert_eq!(zoomed.rotation, view.rotation);
}

#[test]
fn panning_a_turned_view_moves_the_canvas_with_the_pointer() {
    let view = turned(1.1);
    let under_pointer = viewport_to_canvas(view, 100.0, 100.0);
    let panned = pan(view, 40.0, -20.0);
    assert!(close(
        viewport_to_canvas(panned, 140.0, 80.0),
        under_pointer
    ));
}

#[test]
fn rotating_keeps_the_canvas_point_under_the_anchor() {
    let view = turned(0.2);
    let before = viewport_to_canvas(view, 250.0, 150.0);
    let rotated = rotate_around(view, 250.0, 150.0, 0.5);
    assert!(close(viewport_to_canvas(rotated, 250.0, 150.0), before));
    assert!((rotated.rotation - 0.7).abs() < 1e-6);
}

#[test]
fn rotation_stays_within_half_a_turn_either_way() {
    let view = rotate_around(turned(3.0), 0.0, 0.0, 1.0);
    assert!((view.rotation - (4.0 - 2.0 * PI)).abs() < 1e-5);
    assert!((normalise_rotation(-PI) - PI).abs() < 1e-6);
    assert_eq!(normalise_rotation(f32::NAN), 0.0);
}

#[test]
fn fitting_straightens_the_view() {
    assert_eq!(fit_view((800, 600), (1000.0, 700.0), 24.0).rotation, 0.0);
}
