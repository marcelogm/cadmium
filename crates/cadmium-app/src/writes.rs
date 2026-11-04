use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
mod tests {
    use super::*;

    const PAINTING: &str = "/tmp/estudo.cadmium";
    const IMAGE: &str = "/tmp/estudo.png";

    #[test]
    fn a_fresh_painting_has_nothing_unsaved() {
        assert!(!EditCounter::default().has_unsaved_edits());
    }

    #[test]
    fn an_edit_stays_unsaved_until_its_save_succeeds() {
        let mut edits = EditCounter::default();
        edits.record();
        let mark = edits.mark();
        assert!(edits.has_unsaved_edits());
        edits.saved_at(mark);
        assert!(!edits.has_unsaved_edits());
    }

    #[test]
    fn edits_made_while_saving_stay_unsaved_after_the_save() {
        let mut edits = EditCounter::default();
        edits.record();
        let mark = edits.mark();
        edits.record();
        edits.saved_at(mark);
        assert!(edits.has_unsaved_edits());
    }

    #[test]
    fn undoing_back_to_the_saved_painting_still_counts_as_an_edit() {
        let mut edits = EditCounter::default();
        edits.record();
        edits.saved_at(edits.mark());
        edits.record();
        assert!(edits.has_unsaved_edits());
    }

    #[test]
    fn a_new_painting_forgets_the_unsaved_edits_of_the_old_one() {
        let mut edits = EditCounter::default();
        edits.record();
        edits.forget_unsaved();
        assert!(!edits.has_unsaved_edits());
    }

    #[test]
    fn a_second_write_to_the_same_path_is_refused_while_the_first_runs() {
        let mut writes = WritesInFlight::default();
        assert!(writes.begin(Path::new(IMAGE), WriteKind::Image));
        assert!(!writes.begin(Path::new(IMAGE), WriteKind::Image));
        assert!(!writes.begin(Path::new(IMAGE), WriteKind::Painting));
    }

    #[test]
    fn a_path_can_be_written_again_once_its_write_finishes() {
        let mut writes = WritesInFlight::default();
        writes.begin(Path::new(PAINTING), WriteKind::Painting);
        writes.finish(Path::new(PAINTING));
        assert!(writes.is_empty());
        assert!(writes.begin(Path::new(PAINTING), WriteKind::Painting));
    }

    #[test]
    fn saves_and_exports_are_both_waited_for() {
        let mut writes = WritesInFlight::default();
        writes.begin(Path::new(PAINTING), WriteKind::Painting);
        writes.begin(Path::new(IMAGE), WriteKind::Image);
        writes.finish(Path::new(PAINTING));
        assert!(!writes.is_empty());
        writes.finish(Path::new(IMAGE));
        assert!(writes.is_empty());
    }

    #[test]
    fn replacing_the_painting_while_it_is_being_saved_asks_first() {
        let mut writes = WritesInFlight::default();
        writes.begin(Path::new(PAINTING), WriteKind::Painting);
        assert!(work_would_be_lost(false, &writes));
    }

    #[test]
    fn replacing_a_saved_painting_while_it_is_exported_does_not_ask() {
        let mut writes = WritesInFlight::default();
        writes.begin(Path::new(IMAGE), WriteKind::Image);
        assert!(!work_would_be_lost(false, &writes));
    }

    #[test]
    fn replacing_a_painting_with_unsaved_edits_asks_first() {
        assert!(work_would_be_lost(true, &WritesInFlight::default()));
    }
}
