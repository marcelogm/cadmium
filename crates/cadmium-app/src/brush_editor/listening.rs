//! Whether the editor heeds its widgets' signals.

use std::cell::Cell;

/// The editor ignores its widgets while it fills them from the draft (their signals would feed
/// back), and for good once it starts closing: the dialog stays on screen, and clickable,
/// through its close animation, and a second Save there would store the brush twice.
#[derive(Debug, Default)]
pub(super) struct Listening {
    syncing: Cell<bool>,
    closing: Cell<bool>,
}

impl Listening {
    pub(super) fn set_syncing(&self, syncing: bool) {
        self.syncing.set(syncing);
    }

    /// From now on, nothing reaches the editor.
    pub(super) fn stop(&self) {
        self.closing.set(true);
    }

    pub(super) fn heeds(&self) -> bool {
        !self.syncing.get() && !self.closing.get()
    }
}

#[cfg(test)]
mod tests;
