use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;

use super::{Contact, Loading, Squeezing};
use crate::state::AppState;
use crate::tools::Press;

impl AppState {
    pub fn press(&mut self, surface: Surface, sample: StrokeSample, eraser: bool) -> bool {
        let press = if eraser {
            Press::Stroke(Tool::Rag)
        } else {
            self.press_for(surface)
        };
        let began = match press {
            Press::Pan | Press::Nothing | Press::PickColour => false,
            Press::Squeeze => {
                self.begin_squeezing(sample);
                true
            }
            Press::LoadFromPile(tool) => {
                self.begin_loading(tool, sample);
                true
            }
            Press::Stroke(tool) => self.begin_stroke_on(surface, tool, sample),
        };
        self.wake();
        began
    }

    fn begin_squeezing(&mut self, sample: StrokeSample) {
        let squeezing = Squeezing {
            pen: sample,
            nozzle_at: (sample.x, sample.y),
            squeezed: false,
        };
        self.begin_hold(sample, Contact::Squeezing(squeezing));
    }

    fn begin_loading(&mut self, tool: Tool, sample: StrokeSample) {
        let loading = Loading {
            tool,
            origin: (sample.x, sample.y),
            pen: sample,
            colour: None,
            level: None,
        };
        self.begin_hold(sample, Contact::Loading(loading));
        self.session.brush.tool = tool;
    }

    /// Holds only happen on the tray: the tube and loading are the tray's tools.
    fn begin_hold(&mut self, sample: StrokeSample, contact: Contact) {
        self.release();
        self.session.begin_hold(&self.gpu, Surface::Tray);
        self.contact = Some(contact);
        self.hold_clock = None;
        self.last_sample_time_ms = None;
        self.note_samples(std::slice::from_ref(&sample));
    }
}

#[cfg(test)]
mod tests;
