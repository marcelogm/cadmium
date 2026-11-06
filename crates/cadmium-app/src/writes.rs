use std::collections::HashMap;
use std::path::{Path, PathBuf};

mod closing;

pub use closing::{AfterWrite, CloseStep, Closing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditMark(u64);

#[derive(Debug, Default)]
pub struct EditCounter {
    edits: u64,
    saved: u64,
}

impl EditCounter {
    pub fn record(&mut self) {
        self.edits += 1;
    }

    pub fn mark(&self) -> EditMark {
        EditMark(self.edits)
    }

    pub fn saved_at(&mut self, mark: EditMark) {
        self.saved = mark.0;
    }

    pub fn forget_unsaved(&mut self) {
        self.saved = self.edits;
    }

    pub fn has_unsaved_edits(&self) -> bool {
        self.edits != self.saved
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteKind {
    Painting,
    Image,
}

#[derive(Debug, Default)]
pub struct WritesInFlight {
    writes: HashMap<PathBuf, WriteKind>,
}

impl WritesInFlight {
    pub fn begin(&mut self, path: &Path, kind: WriteKind) -> bool {
        if self.writes.contains_key(path) {
            return false;
        }
        self.writes.insert(path.to_path_buf(), kind);
        true
    }

    pub fn finish(&mut self, path: &Path) {
        self.writes.remove(path);
    }

    pub fn is_empty(&self) -> bool {
        self.writes.is_empty()
    }

    pub fn saving_painting(&self) -> bool {
        self.writes
            .values()
            .any(|kind| *kind == WriteKind::Painting)
    }
}

pub fn work_would_be_lost(unsaved_edits: bool, writes: &WritesInFlight) -> bool {
    unsaved_edits || writes.saving_painting()
}

#[cfg(test)]
mod tests;
