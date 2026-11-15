use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::ClearOutcome;

use super::{AppState, Easel, EaselId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DryOutcome {
    NothingWet,
    /// The Palette dialog is up over the painting: nothing is dried.
    PaletteOpen,
    Dried {
        undoable: bool,
    },
}

impl Easel {
    pub fn undo(&mut self, gpu: &GpuContext) -> bool {
        self.release(gpu);
        let undone = self.session.undo(gpu);
        if undone {
            self.measure_brush(gpu);
        }
        undone
    }

    pub fn redo(&mut self, gpu: &GpuContext) -> bool {
        self.release(gpu);
        let redone = self.session.redo(gpu);
        if redone {
            self.measure_brush(gpu);
        }
        redone
    }
}

impl AppState {
    /// Dries the painting's wet paint; not while the Palette dialog hides the painting, where
    /// the drying could be neither seen nor undone.
    pub fn dry(&mut self) -> DryOutcome {
        if self.is_editing_palette() {
            return DryOutcome::PaletteOpen;
        }
        self.release(EaselId::Painting);
        let session = &mut self.painting.session;
        session.flush(&self.gpu);
        if session.canvas.wet_tiles.is_empty() {
            return DryOutcome::NothingWet;
        }
        let undoable = session.dry(&self.gpu);
        self.edits.record();
        self.wake();
        DryOutcome::Dried { undoable }
    }

    /// Clears the margin around the painting of wet paint, dry paint and graphite; not while
    /// the Palette dialog hides the painting, where the clearing could be neither seen nor
    /// undone.
    pub fn clear_margin(&mut self) -> ClearOutcome {
        if self.is_editing_palette() {
            return ClearOutcome::NothingToClear;
        }
        self.release(EaselId::Painting);
        let outcome = self.painting.session.clear_margin(&self.gpu);
        if matches!(outcome, ClearOutcome::Cleared { .. }) {
            self.edits.record();
            self.wake();
        }
        outcome
    }

    pub fn undo(&mut self, id: EaselId) -> bool {
        self.release(id);
        let gpu = self.gpu.clone();
        let undone = self.easel_mut(id).undo(&gpu);
        self.after_history_step(undone)
    }

    pub fn redo(&mut self, id: EaselId) -> bool {
        self.release(id);
        let gpu = self.gpu.clone();
        let redone = self.easel_mut(id).redo(&gpu);
        self.after_history_step(redone)
    }

    fn after_history_step(&mut self, stepped: bool) -> bool {
        if stepped {
            self.edits.record();
        }
        self.wake();
        stepped
    }

    pub fn can_undo(&self, id: EaselId) -> bool {
        self.easel(id).session.history.can_undo()
    }

    pub fn can_redo(&self, id: EaselId) -> bool {
        self.easel(id).session.history.can_redo()
    }

    pub fn history_is_enabled(&self) -> bool {
        self.painting.session.history.is_enabled()
    }
}

#[cfg(test)]
mod tests;
