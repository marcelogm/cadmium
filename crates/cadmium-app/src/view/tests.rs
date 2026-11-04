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
