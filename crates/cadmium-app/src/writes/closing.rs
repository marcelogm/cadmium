//! Closing the window while paintings are being written: wait for the writes, ask before
//! dropping unsaved edits, and let the painter give up on writes that hang.

use std::path::Path;

use super::{WriteKind, WritesInFlight, work_would_be_lost};

/// What the window does when asked to close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseStep {
    Close,
    /// Writes are still running: stay open, close when they finish, and offer to quit
    /// anyway if they hang.
    WaitForWrites,
    /// Unsaved edits would be lost: ask whether to discard them.
    Ask,
    /// The question is already on screen.
    AlreadyAsking,
}

/// What follows a write finishing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterWrite {
    StillWriting,
    Idle,
    /// The window was asked to close while writing, and the last write is done.
    CloseNow,
}

#[derive(Debug, Default)]
pub struct Closing {
    writes: WritesInFlight,
    close_after_writes: bool,
    abandon_writes: bool,
    close_confirmed: bool,
    asking: bool,
}

impl Closing {
    pub fn begin_write(&mut self, path: &Path, kind: WriteKind) -> bool {
        self.writes.begin(path, kind)
    }

    pub fn finish_write(&mut self, path: &Path) -> AfterWrite {
        self.writes.finish(path);
        if !self.writes.is_empty() {
            AfterWrite::StillWriting
        } else if std::mem::take(&mut self.close_after_writes) {
            AfterWrite::CloseNow
        } else {
            AfterWrite::Idle
        }
    }

    /// `unsaved_edits` is read only when the answer depends on it.
    pub fn request_close(&mut self, unsaved_edits: impl FnOnce() -> bool) -> CloseStep {
        if self.abandon_writes {
            return CloseStep::Close;
        }
        if !self.writes.is_empty() {
            self.close_after_writes = true;
            return CloseStep::WaitForWrites;
        }
        if self.close_confirmed || !unsaved_edits() {
            return CloseStep::Close;
        }
        if std::mem::replace(&mut self.asking, true) {
            CloseStep::AlreadyAsking
        } else {
            CloseStep::Ask
        }
    }

    pub fn discard_answered(&mut self, discard: bool) {
        self.asking = false;
        if discard {
            self.close_confirmed = true;
        }
    }

    /// Close without asking, as when the GPU is lost.
    pub fn confirm(&mut self) {
        self.close_confirmed = true;
    }

    pub fn abandon_writes(&mut self) {
        self.abandon_writes = true;
    }

    /// A close is waiting on writes that have not finished.
    pub fn still_waiting_for_writes(&self) -> bool {
        self.close_after_writes && !self.writes.is_empty()
    }

    pub fn work_would_be_lost(&self, unsaved_edits: bool) -> bool {
        work_would_be_lost(unsaved_edits, &self.writes)
    }
}

#[cfg(test)]
mod tests;
