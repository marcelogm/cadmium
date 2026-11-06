use std::f64::consts::{PI, TAU};

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

/// The canvas offset a screen offset (`x`, `y`) shows when the picture is turned clockwise
/// by `rotation`: `R(-rotation)` applied to it.
fn unturn(rotation: f32, x: f64, y: f64) -> (f64, f64) {
    let (s, c) = f64::from(rotation).sin_cos();
    (c * x + s * y, -s * x + c * y)
}

/// The screen offset a canvas offset (`x`, `y`) appears at: `R(rotation)` applied to it.
#[cfg(test)]
fn turn(rotation: f32, x: f64, y: f64) -> (f64, f64) {
    let (s, c) = f64::from(rotation).sin_cos();
    (c * x - s * y, s * x + c * y)
}

/// `rotation` brought into (-π, π]; a non-finite one is upright.
pub fn normalise_rotation(rotation: f32) -> f32 {
    if !rotation.is_finite() {
        return 0.0;
    }
    let turned = f64::from(rotation).rem_euclid(TAU);
    (if turned > PI { turned - TAU } else { turned }) as f32
}

/// The canvas point screen px (`x_px`, `y_px`) shows.
pub fn viewport_to_canvas(view: ViewTransform, x_px: f64, y_px: f64) -> (f64, f64) {
    let zoom = f64::from(view.zoom);
    let (dx, dy) = unturn(view.rotation, x_px / zoom, y_px / zoom);
    (f64::from(view.origin_x) + dx, f64::from(view.origin_y) + dy)
}

/// The screen px where canvas point (`x`, `y`) appears; the inverse of `viewport_to_canvas`.
#[cfg(test)]
fn canvas_to_viewport(view: ViewTransform, x: f64, y: f64) -> (f64, f64) {
    let zoom = f64::from(view.zoom);
    let (dx, dy) = turn(
        view.rotation,
        x - f64::from(view.origin_x),
        y - f64::from(view.origin_y),
    );
    (dx * zoom, dy * zoom)
}

/// The view at `zoom` and `rotation` that shows canvas point `anchor` at screen px
/// (`x_px`, `y_px`).
fn anchored(anchor: (f64, f64), x_px: f64, y_px: f64, zoom: f32, rotation: f32) -> ViewTransform {
    let zoom_f64 = f64::from(zoom);
    let (dx, dy) = unturn(rotation, x_px / zoom_f64, y_px / zoom_f64);
    ViewTransform {
        origin_x: (anchor.0 - dx) as f32,
        origin_y: (anchor.1 - dy) as f32,
        zoom,
        rotation,
    }
}

/// The view at `zoom` that keeps the canvas point under screen px (`x_px`, `y_px`) in place.
pub fn zoom_around(view: ViewTransform, x_px: f64, y_px: f64, zoom: f32) -> ViewTransform {
    let anchor = viewport_to_canvas(view, x_px, y_px);
    anchored(anchor, x_px, y_px, clamp_zoom(zoom), view.rotation)
}

/// The view turned by `delta` more radians clockwise about screen px (`x_px`, `y_px`).
pub fn rotate_around(view: ViewTransform, x_px: f64, y_px: f64, delta: f32) -> ViewTransform {
    let anchor = viewport_to_canvas(view, x_px, y_px);
    let rotation = normalise_rotation(view.rotation + delta);
    anchored(anchor, x_px, y_px, view.zoom, rotation)
}

/// The view moved by a screen-px drag, so the canvas follows the pointer.
pub fn pan(view: ViewTransform, dx_px: f64, dy_px: f64) -> ViewTransform {
    let zoom = f64::from(view.zoom);
    let (dx, dy) = unturn(view.rotation, dx_px / zoom, dy_px / zoom);
    ViewTransform {
        origin_x: (f64::from(view.origin_x) - dx) as f32,
        origin_y: (f64::from(view.origin_y) - dy) as f32,
        ..view
    }
}

/// One change a navigation gesture or key makes to the view, in widget px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewStep {
    Pan { dx: f64, dy: f64 },
    Zoom { anchor: (f64, f64), factor: f32 },
    Rotate { centre: (f64, f64), delta: f32 },
}

/// `view` with its zoom in range and its rotation in (-π, π].
pub fn settle(view: ViewTransform) -> ViewTransform {
    ViewTransform {
        zoom: clamp_zoom(view.zoom),
        rotation: normalise_rotation(view.rotation),
        ..view
    }
}

/// `view` after `change`, whose widget px are `scale` device px each.
pub fn step(view: ViewTransform, change: ViewStep, scale: f64) -> ViewTransform {
    settle(match change {
        ViewStep::Pan { dx, dy } => pan(view, dx * scale, dy * scale),
        ViewStep::Zoom {
            anchor: (x, y),
            factor,
        } => zoom_around(view, x * scale, y * scale, view.zoom * factor),
        ViewStep::Rotate {
            centre: (x, y),
            delta,
        } => rotate_around(view, x * scale, y * scale, delta),
    })
}

#[cfg(test)]
mod tests;
