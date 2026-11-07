use cadmium_engine::surface::{TRAY_HEIGHT, TRAY_WIDTH};

use crate::presenter::tray_view;
use crate::view;

pub(super) fn tray_point(
    viewport_px: (u32, u32),
    device_scale: f64,
    widget_x: f64,
    widget_y: f64,
) -> (f64, f64) {
    view::viewport_to_canvas(
        tray_view(viewport_px),
        widget_x * device_scale,
        widget_y * device_scale,
    )
}

pub(super) fn is_on_tray((x, y): (f64, f64)) -> bool {
    (0.0..f64::from(TRAY_WIDTH)).contains(&x) && (0.0..f64::from(TRAY_HEIGHT)).contains(&y)
}

/// The least height the tray asks for; it fills whatever its pane gives it.
pub(super) const TRAY_MIN_HEIGHT: i32 = 120;

pub(super) fn tray_min_height() -> i32 {
    TRAY_MIN_HEIGHT
}

pub(super) fn tray_screen_zoom(viewport_px: (u32, u32), device_scale: f64) -> f32 {
    tray_view(viewport_px).zoom / device_scale as f32
}

#[cfg(test)]
mod tests;
