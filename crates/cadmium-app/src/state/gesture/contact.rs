use cadmium_engine::gpu::GpuContext;
use cadmium_engine::stroke::StrokeSample;

use crate::state::{AppState, Easel, EaselId};

impl Easel {
    pub fn is_stroking(&self) -> bool {
        self.stroking
    }

    /// Feeds the running stroke; false when there is none, or nothing to feed it.
    pub fn motion(&mut self, samples: &[StrokeSample]) -> bool {
        if !self.stroking || samples.is_empty() {
            return false;
        }
        self.session.add_samples(samples);
        true
    }

    /// Ends the running stroke; true when there was one.
    pub fn release(&mut self, gpu: &GpuContext) -> bool {
        if !std::mem::take(&mut self.stroking) {
            return false;
        }
        self.finish_stroke(gpu);
        true
    }
}

impl AppState {
    /// Whether a stroke runs on any easel.
    pub fn is_stroking(&self) -> bool {
        self.painting.is_stroking()
    }

    pub fn motion(&mut self, id: EaselId, samples: &[StrokeSample]) {
        self.add_samples(id, samples);
    }

    pub fn add_samples(&mut self, id: EaselId, samples: &[StrokeSample]) {
        if !self.easel_mut(id).motion(samples) {
            return;
        }
        self.note_samples(samples);
        self.wake();
    }

    pub fn release(&mut self, id: EaselId) {
        let gpu = self.gpu.clone();
        if !self.easel_mut(id).release(&gpu) {
            return;
        }
        self.edits.record();
        self.wake();
    }
}

#[cfg(test)]
mod tests;
