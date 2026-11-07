use crate::settings::{divider_position, tray_share_at};

/// A tray dragged lower than this, in widget px, closes into its bar when let go.
pub(super) const CLOSE_BELOW_PX: i32 = 120;
/// How far up, in widget px, a drag on the closed bar goes before it opens the tray.
pub(super) const OPEN_DRAG_PX: f64 = 8.0;

/// `position` kept where the open tray, below a `handle_px` handle in a paned `height` tall,
/// is at least `tray_min_px` tall; the start child's `min_position` wins when both can't fit.
pub(super) fn clamp_open_position(
    position: i32,
    min_position: i32,
    height: i32,
    handle_px: i32,
    tray_min_px: i32,
) -> i32 {
    let lowest = height - handle_px - tray_min_px;
    position.min(lowest).max(min_position)
}

/// Whether a drag on the closed tray's bar, now `offset_y` from where it began, opens it.
pub(super) fn bar_drag_opens(offset_y: f64) -> bool {
    offset_y <= -OPEN_DRAG_PX
}

/// The tray divider, kept as the tray's share of the paned: the position follows the share
/// whenever the paned's height changes, and only the painter's own moves change the share.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TrayDivider {
    share: f32,
    /// The height the divider was last placed for; GTK's own moves at another height are
    /// layout, not the painter.
    laid_out_height: Option<i32>,
    placing: bool,
    /// A press on the divider's handle is down.
    grabbed: bool,
    /// The painter has the divider low enough that letting go closes the tray.
    closing: bool,
}

impl Default for TrayDivider {
    fn default() -> Self {
        TrayDivider::new(crate::settings::DEFAULT_TRAY_SHARE)
    }
}

impl TrayDivider {
    pub(super) fn new(share: f32) -> Self {
        TrayDivider {
            share,
            laid_out_height: None,
            placing: false,
            grabbed: false,
            closing: false,
        }
    }

    pub(super) fn share(&self) -> f32 {
        self.share
    }

    /// The position for a paned `height` tall, or `None` before it has one. Moves reported
    /// until [`TrayDivider::end_place`] are this placing's own.
    pub(super) fn place(&mut self, height: i32) -> Option<i32> {
        if height <= 0 {
            return None;
        }
        self.laid_out_height = Some(height);
        self.placing = true;
        Some(divider_position(self.share, height))
    }

    pub(super) fn end_place(&mut self) {
        self.placing = false;
    }

    /// Takes no move as the painter's until the divider is placed again.
    pub(super) fn forget_layout(&mut self) {
        self.laid_out_height = None;
        self.closing = false;
    }

    /// Follows the divider to `position` in a paned `height` tall. Low enough to close, the
    /// share stays the last good one, for the tray to open at again, and letting go of a
    /// grabbed handle there closes the tray.
    pub(super) fn follow_move(&mut self, position: i32, height: i32) {
        if self.placing || height <= 0 || self.laid_out_height != Some(height) {
            return;
        }
        let low = height - position < CLOSE_BELOW_PX;
        self.closing = low && self.grabbed;
        if !low {
            self.share = tray_share_at(position, height);
        }
    }

    /// A press began on the divider's handle; only its drag may close the tray.
    pub(super) fn grab(&mut self) {
        self.grabbed = true;
    }

    /// The press ended: whether letting go there closes the tray.
    pub(super) fn released(&mut self) -> bool {
        self.grabbed = false;
        std::mem::take(&mut self.closing)
    }

    /// The press was cancelled: nothing closes.
    pub(super) fn cancel(&mut self) {
        self.grabbed = false;
        self.closing = false;
    }
}

#[cfg(test)]
mod tests;
