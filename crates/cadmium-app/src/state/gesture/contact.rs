use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::Surface;

use super::{Contact, Loading, Squeezing};
use crate::state::AppState;

impl AppState {
    pub fn is_stroking(&self) -> bool {
        self.contact.is_some()
    }

    pub fn holds_on(&self, surface: Surface) -> bool {
        self.hold_surface() == Some(surface)
    }

    pub fn contact_on(&self, surface: Surface) -> bool {
        self.contact_surface() == Some(surface)
    }

    pub(super) fn hold_surface(&self) -> Option<Surface> {
        match self.contact? {
            Contact::Stroke { .. } => None,
            _ => self.contact_surface(),
        }
    }

    fn contact_surface(&self) -> Option<Surface> {
        match self.contact? {
            Contact::Stroke { surface } | Contact::Squeezing(Squeezing { surface, .. }) => {
                Some(surface)
            }
            Contact::Loading(_) => Some(Surface::Tray),
        }
    }

    pub fn motion(&mut self, samples: &[StrokeSample]) {
        let Some(&pen) = samples.last() else {
            return;
        };
        match self.contact {
            None => {}
            Some(Contact::Stroke { .. }) => self.add_samples(samples),
            Some(Contact::Squeezing(squeezing)) => self.squeezing_motion(squeezing, pen, samples),
            Some(Contact::Loading(loading)) => self.loading_motion(loading, pen, samples),
        }
    }

    #[expect(clippy::excessive_nesting)]
    pub fn release(&mut self) {
        let Some(contact) = self.contact.take() else {
            return;
        };
        match contact {
            Contact::Stroke { .. } => self.finish_stroke(),
            Contact::Squeezing(Squeezing { squeezed, .. }) => {
                self.session.end_hold();
                if squeezed {
                    self.edits.record();
                }
            }
            Contact::Loading(Loading { tool, colour, .. }) => {
                self.session.end_hold();
                self.finish_loading(tool, colour.is_some());
            }
        }
        self.wake();
    }
}

#[cfg(test)]
mod tests;
