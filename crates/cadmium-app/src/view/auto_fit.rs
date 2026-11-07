//! When a surface (the canvas or the tray) fits itself to its widget: on a new size or
//! scale, unless the painter has moved the view, and not in the middle of a stroke.

#[derive(Clone, Copy, Debug, Default)]
pub struct AutoFit {
    user_moved: bool,
    held_back: bool,
}

impl AutoFit {
    /// The widget's size or scale changed; `sized` when it has a size. Whether to fit now.
    /// `stroking` is asked only when a fit is due.
    pub(crate) fn on_size_change(&mut self, sized: bool, stroking: impl FnOnce() -> bool) -> bool {
        !self.user_moved && sized && self.fit_unless(stroking)
    }

    /// A frame is drawn: whether a fit held back by a stroke can happen now. Moving the view
    /// pauses a held-back fit rather than dropping it; it happens after the next fit by hand.
    pub(crate) fn on_frame(&mut self, stroking: impl FnOnce() -> bool) -> bool {
        self.held_back && !self.user_moved && self.fit_unless(stroking)
    }

    /// The painter moved the view; auto-fit stays off until the next fit.
    pub(crate) fn moved_by_user(&mut self) {
        self.user_moved = true;
    }

    /// The view was fitted, by the widget or by the painter.
    pub(crate) fn fitted(&mut self) {
        self.user_moved = false;
    }

    fn fit_unless(&mut self, stroking: impl FnOnce() -> bool) -> bool {
        self.held_back = stroking();
        !self.held_back
    }
}

#[cfg(test)]
mod tests;
