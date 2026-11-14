use super::AppState;
use super::recipe::PigmentClick;

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
        self.choose_pigment(slot, click);
    }
}

#[cfg(test)]
mod tests;
