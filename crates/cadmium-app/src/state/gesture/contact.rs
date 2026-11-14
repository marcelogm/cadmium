use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::Surface;

use super::Contact;
use crate::state::AppState;

impl AppState {
    pub fn is_stroking(&self) -> bool {
        self.contact.is_some()
    }

    pub fn contact_on(&self, surface: Surface) -> bool {
        self.contact_surface() == Some(surface)
    }

    fn contact_surface(&self) -> Option<Surface> {
        self.contact.map(|Contact::Stroke { surface }| surface)
    }

    pub fn motion(&mut self, samples: &[StrokeSample]) {
        self.add_samples(samples);
    }

    pub fn release(&mut self) {
        if self.contact.take().is_none() {
            return;
        }
        self.finish_stroke();
        self.wake();
    }
}

#[cfg(test)]
mod tests;
