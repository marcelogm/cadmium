use cadmium_engine::present::ViewTransform;

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
        rotation: 0.0,
    }
}

/// The canvas covering the whole viewport, centred: scaled by the larger of the two ratios,
/// so it fills the viewport edge to edge and the overflow is cropped.
pub fn cover_view(canvas_size: (u32, u32), viewport_px: (u32, u32)) -> ViewTransform {
    let (canvas_width, canvas_height) = (f64::from(canvas_size.0), f64::from(canvas_size.1));
    let (width, height) = (f64::from(viewport_px.0), f64::from(viewport_px.1));
    let zoom = clamp_zoom((width / canvas_width).max(height / canvas_height) as f32);
    let zoom_f64 = f64::from(zoom);
    ViewTransform {
        origin_x: (canvas_width / 2.0 - width / 2.0 / zoom_f64) as f32,
        origin_y: (canvas_height / 2.0 - height / 2.0 / zoom_f64) as f32,
        zoom,
        rotation: 0.0,
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
        rotation: view.rotation,
    }
}

pub fn pan(view: ViewTransform, dx_px: f64, dy_px: f64) -> ViewTransform {
    let zoom = f64::from(view.zoom);
    ViewTransform {
        origin_x: (f64::from(view.origin_x) - dx_px / zoom) as f32,
        origin_y: (f64::from(view.origin_y) - dy_px / zoom) as f32,
        zoom: view.zoom,
        rotation: view.rotation,
    }
}

#[cfg(test)]
mod tests;
