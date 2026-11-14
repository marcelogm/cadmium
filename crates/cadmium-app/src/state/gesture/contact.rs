use cadmium_engine::stroke::StrokeSample;

use crate::state::AppState;

impl AppState {
    pub fn is_stroking(&self) -> bool {
        self.stroking
    }

    pub fn motion(&mut self, samples: &[StrokeSample]) {
        self.add_samples(samples);
    }

    pub fn release(&mut self) {
        if !std::mem::take(&mut self.stroking) {
            return;
        }
        self.finish_stroke();
        self.wake();
    }
}

#[cfg(test)]
mod tests;
