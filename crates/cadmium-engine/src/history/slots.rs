use super::{History, Slot, Step};

impl History {
    pub(super) fn allocate(&mut self) -> Option<Slot> {
        while self.free_slots.is_empty() {
            let oldest = self.undo_steps.pop_front()?;
            self.release(oldest);
        }
        self.free_slots.pop()
    }

    pub(super) fn reserve_after_slots(&mut self, step: &mut Step) -> bool {
        step.tiles
            .iter_mut()
            .filter(|saved| saved.after.is_none())
            .all(|saved| {
                saved.after = self.allocate();
                saved.after.is_some()
            })
    }

    pub fn clear(&mut self) {
        if let Some(step) = self.current.take() {
            self.release(step);
        }
        self.drop_undo_steps();
        self.drop_redo_steps();
    }

    pub(super) fn abandon(&mut self, step: &mut Step) {
        step.complete = false;
        self.free_slots.extend(
            std::mem::take(&mut step.tiles)
                .iter()
                .map(|saved| saved.before),
        );
        self.drop_undo_steps();
        self.drop_redo_steps();
    }

    fn drop_undo_steps(&mut self) {
        while let Some(step) = self.undo_steps.pop_front() {
            self.release(step);
        }
    }

    pub(super) fn release(&mut self, step: Step) {
        self.free_slots.extend(step.slots());
    }

    pub(super) fn drop_redo_steps(&mut self) {
        while let Some(step) = self.redo_steps.pop() {
            self.release(step);
        }
    }

    pub(super) fn trim_to_max_steps(&mut self) {
        while self.undo_steps.len() > self.max_steps
            && let Some(oldest) = self.undo_steps.pop_front()
        {
            self.release(oldest);
        }
    }
}
