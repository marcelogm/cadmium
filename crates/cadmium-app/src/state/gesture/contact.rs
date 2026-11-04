use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::Surface;

use super::{Contact, LOAD_CANCEL_SCREEN_PX, load_cancelled};
use crate::state::{AppState, StrokePreparation};

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
            Contact::Stroke { surface } | Contact::Squeezing { surface, .. } => Some(surface),
            Contact::Loading { .. } => Some(Surface::Tray),
        }
    }

    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
    pub fn motion(&mut self, samples: &[StrokeSample]) {
        let Some(&pen) = samples.last() else {
            return;
        };
        match self.contact {
            None => return,
            Some(Contact::Stroke { .. }) => {
                self.add_samples(samples);
                return;
            }
            Some(Contact::Squeezing {
                surface,
                nozzle_at,
                squeezed,
                ..
            }) => {
                self.contact = Some(Contact::Squeezing {
                    surface,
                    pen,
                    nozzle_at,
                    squeezed,
                });
            }
            Some(Contact::Loading {
                tool,
                origin,
                colour,
                level,
                ..
            }) => {
                let threshold_px = LOAD_CANCEL_SCREEN_PX / f64::from(self.tray_zoom);
                let cancelled_at = samples
                    .iter()
                    .position(|sample| load_cancelled(origin, (sample.x, sample.y), threshold_px));
                if let Some(index) = cancelled_at {
                    self.begin_prepared_stroke(
                        Surface::Tray,
                        tool,
                        samples[index],
                        StrokePreparation::None,
                    );
                    self.add_samples(&samples[index + 1..]);
                    return;
                }
                self.contact = Some(Contact::Loading {
                    tool,
                    origin,
                    pen,
                    colour,
                    level,
                });
            }
        }
        self.note_samples(samples);
        self.wake();
    }

    #[expect(clippy::excessive_nesting)]
    pub fn release(&mut self) {
        let Some(contact) = self.contact.take() else {
            return;
        };
        match contact {
            Contact::Stroke { .. } => self.finish_stroke(),
            Contact::Squeezing { squeezed, .. } => {
                self.session.end_hold();
                if squeezed {
                    self.edits.record();
                }
            }
            Contact::Loading { tool, colour, .. } => {
                self.session.end_hold();
                self.finish_loading(tool, colour.is_some());
            }
        }
        self.wake();
    }
}
