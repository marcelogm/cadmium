use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::Surface;

use super::Contact;
use crate::state::AppState;

impl AppState {
    pub fn is_stroking(&self) -> bool {
        self.contact.is_some()
    }

    /// Holds only happen on the tray.
    pub fn holds_on(&self, surface: Surface) -> bool {
        surface == Surface::Tray
            && matches!(
                self.contact,
                Some(Contact::Loading(_) | Contact::Squeezing(_))
            )
    }

    pub fn contact_on(&self, surface: Surface) -> bool {
        self.contact_surface() == Some(surface)
    }

    fn contact_surface(&self) -> Option<Surface> {
        match self.contact? {
            Contact::Stroke { surface } => Some(surface),
            Contact::Loading(_) | Contact::Squeezing(_) => Some(Surface::Tray),
        }
    }

    pub fn motion(&mut self, samples: &[StrokeSample]) {
        let Some(&pen) = samples.last() else {
            return;
        };
        match self.contact {
            None => {}
            Some(Contact::Stroke { .. }) => self.add_samples(samples),
            Some(Contact::Squeezing(squeezing)) => self.follow_squeezing(squeezing, pen, samples),
            Some(Contact::Loading(loading)) => self.follow_loading(loading, pen, samples),
        }
    }

    pub fn release(&mut self) {
        let Some(contact) = self.contact.take() else {
            return;
        };
        match contact {
            Contact::Stroke { .. } => self.finish_stroke(),
            Contact::Squeezing(squeezing) => self.finish_squeezing(squeezing.squeezed),
            Contact::Loading(loading) => {
                self.finish_loading(loading.tool, loading.colour.is_some())
            }
        }
        self.wake();
    }
}

#[cfg(test)]
mod tests;
