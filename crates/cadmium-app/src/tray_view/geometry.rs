use cadmium_engine::present::ViewTransform;
use cadmium_engine::surface::{TRAY_HEIGHT, TRAY_WIDTH};

use crate::view;

/// The least height the tray takes, in widget px.
pub(super) const TRAY_MIN_HEIGHT: i32 = 120;
/// The height the tray asks for: its sheet opens to it.
pub(super) const TRAY_NATURAL_HEIGHT: i32 = 320;

/// The tray point under widget point (`widget_x`, `widget_y`) with the tray shown by `view`
/// on a screen of `device_scale` device px per widget px.
pub(super) fn tray_point(
    view: ViewTransform,
    device_scale: f64,
    widget_x: f64,
    widget_y: f64,
) -> (f64, f64) {
    view::viewport_to_canvas(view, widget_x * device_scale, widget_y * device_scale)
}

pub(super) fn is_on_tray((x, y): (f64, f64)) -> bool {
    (0.0..f64::from(TRAY_WIDTH)).contains(&x) && (0.0..f64::from(TRAY_HEIGHT)).contains(&y)
}

/// The tray filling a `viewport_px` widget edge to edge, centred, the overflow cropped.
pub(super) fn tray_cover(viewport_px: (u32, u32)) -> ViewTransform {
    view::cover_view((TRAY_WIDTH, TRAY_HEIGHT), viewport_px)
}

#[cfg(test)]
mod tests;
