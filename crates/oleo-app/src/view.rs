use oleo_engine::present::ViewTransform;

pub const MIN_ZOOM: f32 = 0.05;
pub const MAX_ZOOM: f32 = 16.0;

pub fn clamp_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.clamp(MIN_ZOOM, MAX_ZOOM)
    } else {
        1.0
    }
}

pub fn fit_view(canvas_size: (u32, u32), viewport_px: (f64, f64), margin_px: f64) -> ViewTransform {
    let (canvas_width, canvas_height) = (f64::from(canvas_size.0), f64::from(canvas_size.1));
    let usable_width = (viewport_px.0 - 2.0 * margin_px).max(1.0);
    let usable_height = (viewport_px.1 - 2.0 * margin_px).max(1.0);
    let zoom = clamp_zoom((usable_width / canvas_width).min(usable_height / canvas_height) as f32);
    let zoom_f64 = f64::from(zoom);
    ViewTransform {
        origin_x: (canvas_width / 2.0 - viewport_px.0 / 2.0 / zoom_f64) as f32,
        origin_y: (canvas_height / 2.0 - viewport_px.1 / 2.0 / zoom_f64) as f32,
        zoom,
    }
}

pub fn viewport_to_canvas(view: ViewTransform, x_px: f64, y_px: f64) -> (f64, f64) {
    let zoom = f64::from(view.zoom);
    (
        f64::from(view.origin_x) + x_px / zoom,
        f64::from(view.origin_y) + y_px / zoom,
    )
}

pub fn zoom_around(view: ViewTransform, x_px: f64, y_px: f64, zoom: f32) -> ViewTransform {
    let (anchor_x, anchor_y) = viewport_to_canvas(view, x_px, y_px);
    let zoom = clamp_zoom(zoom);
    let zoom_f64 = f64::from(zoom);
    ViewTransform {
        origin_x: (anchor_x - x_px / zoom_f64) as f32,
        origin_y: (anchor_y - y_px / zoom_f64) as f32,
        zoom,
    }
}

pub fn pan(view: ViewTransform, dx_px: f64, dy_px: f64) -> ViewTransform {
    let zoom = f64::from(view.zoom);
    ViewTransform {
        origin_x: (f64::from(view.origin_x) - dx_px / zoom) as f32,
        origin_y: (f64::from(view.origin_y) - dy_px / zoom) as f32,
        zoom: view.zoom,
    }
}

#[cfg(test)]
mod tests {
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
}
