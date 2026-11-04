use cadmium_engine::session::ClearOutcome;

use super::AppState;
use super::recipe::PigmentClick;
use crate::tools::UiTool;

impl AppState {
    pub fn slot_count(&self) -> usize {
        self.session.canvas.spec.pigment_slots.len()
    }

    pub fn add_pile(&mut self, id: &str) -> Result<usize, String> {
        let slot = self.session.add_slot(&self.gpu, id)?;
        self.edits.record();
        self.click_pile(slot, PigmentClick::CleanAndLoad);
        self.wake();
        Ok(slot)
    }

    pub fn click_pile(&mut self, slot: usize, click: PigmentClick) {
        match self.ui_tool {
            UiTool::Tube => self.set_tube_slot(slot),
            UiTool::Move | UiTool::Paint(_) => self.choose_pigment(slot, click),
        }
    }

    pub fn set_tube_slot(&mut self, slot: usize) {
        if slot < self.slot_count() {
            self.tube_slot = slot;
        }
    }

    pub fn clear_tray(&mut self) {
        self.release();
        if let ClearOutcome::Cleared { .. } = self.session.clear_tray(&self.gpu) {
            self.edits.record();
        }
        self.wake();
    }
}

#[cfg(test)]
mod tests;
