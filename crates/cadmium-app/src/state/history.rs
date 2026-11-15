use cadmium_engine::gpu::GpuContext;

use super::{AppState, Easel, EaselId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DryOutcome {
    NothingWet,
    Dried { undoable: bool },
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
    pub fn dry(&mut self) -> DryOutcome {
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
