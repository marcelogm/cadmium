use super::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DryOutcome {
    NothingWet,
    Dried { undoable: bool },
}

impl AppState {
    pub fn dry(&mut self) -> DryOutcome {
        self.release();
        self.session.flush(&self.gpu);
        if self.session.canvas.wet_tiles.is_empty() {
            return DryOutcome::NothingWet;
        }
        let undoable = self.session.dry(&self.gpu);
        self.edits.record();
        self.wake();
        DryOutcome::Dried { undoable }
    }

    pub fn undo(&mut self) -> bool {
        self.release();
        let undone = self.session.undo(&self.gpu);
        if undone {
            self.edits.record();
        }
        self.wake();
        undone
    }

    pub fn redo(&mut self) -> bool {
        self.release();
        let redone = self.session.redo(&self.gpu);
        if redone {
            self.edits.record();
        }
        self.wake();
        redone
    }

    pub fn can_undo(&self) -> bool {
        self.session.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.session.history.can_redo()
    }

    pub fn history_is_enabled(&self) -> bool {
        self.session.history.is_enabled()
    }
}

#[cfg(test)]
mod tests;
