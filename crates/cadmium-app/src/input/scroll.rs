//! What a scroll or a pinch does to the canvas view.

/// Zoom factor of one scroll step.
const ZOOM_PER_SCROLL_STEP: f32 = 1.1;
/// Surface px that count as one scroll step.
const SURFACE_SCROLL_PER_STEP: f64 = 15.0;
/// Widget px a wheel notch pans by.
const WHEEL_PAN_STEP: f64 = 48.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScrollUnit {
    /// Mouse wheel notches.
    Wheel,
    /// Touchpad and other smooth scrolling, in surface px.
    Surface,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ScrollAction {
    /// Move the view by these widget px.
    Pan { dx: f64, dy: f64 },
    /// Zoom about the pointer by this factor.
    Zoom { factor: f32 },
    /// Leave the scroll to the widgets underneath.
    Pass,
}

/// A scroll of (`dx`, `dy`) in `unit`: with Shift it pans, otherwise an up or down scroll
/// zooms; a sideways-only scroll without Shift is left alone.
pub(super) fn scroll_action(unit: ScrollUnit, shift: bool, dx: f64, dy: f64) -> ScrollAction {
    if shift {
        let step = match unit {
            ScrollUnit::Wheel => WHEEL_PAN_STEP,
            ScrollUnit::Surface => 1.0,
        };
        let horizontal = if dx == 0.0 { dy } else { dx };
        let vertical = if dx == 0.0 { 0.0 } else { dy };
        return ScrollAction::Pan {
            dx: -horizontal * step,
            dy: -vertical * step,
        };
    }
    if dy == 0.0 {
        return ScrollAction::Pass;
    }
    let steps = match unit {
        ScrollUnit::Wheel => dy,
        ScrollUnit::Surface => dy / SURFACE_SCROLL_PER_STEP,
    };
    ScrollAction::Zoom {
        factor: ZOOM_PER_SCROLL_STEP.powf(-steps as f32),
    }
}

/// The zoom factor for a pinch now at `scale` that was at `previous`; none while either has
/// no size.
pub(super) fn pinch_factor(previous: f64, scale: f64) -> Option<f32> {
    (previous > 0.0 && scale > 0.0).then(|| (scale / previous) as f32)
}

#[cfg(test)]
mod tests;
