use crate::settings::{divider_position, tray_share_at};

/// The tray divider, kept as the tray's share of the paned: the position follows the share
/// whenever the paned's height changes, and only the painter's own moves change the share.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TrayDivider {
    share: f32,
    /// The height the divider was last placed for; GTK's own moves at another height are
    /// layout, not the painter.
    laid_out_height: Option<i32>,
    placing: bool,
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
    }

    /// Follows the divider to `position` in a paned `height` tall.
    pub(super) fn follow_move(&mut self, position: i32, height: i32) {
        if !self.placing && height > 0 && self.laid_out_height == Some(height) {
            self.share = tray_share_at(position, height);
        }
    }
}

#[cfg(test)]
mod tests;
