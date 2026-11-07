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
                self.begin_squeezing(surface, sample);
                true
            }
            Press::LoadFromPile(tool) => {
                self.begin_loading(surface, tool, sample);
                true
            }
            Press::Stroke(tool) => {
                self.begin_stroke_on(surface, tool, sample);
                true
            }
        };
        self.wake();
        began
    }

    fn begin_squeezing(&mut self, surface: Surface, sample: StrokeSample) {
        let squeezing = Squeezing {
            surface,
            pen: sample,
            nozzle_at: (sample.x, sample.y),
            squeezed: false,
        };
        self.begin_hold(surface, sample, Contact::Squeezing(squeezing));
    }

    fn begin_loading(&mut self, surface: Surface, tool: Tool, sample: StrokeSample) {
        let loading = Loading {
            tool,
            origin: (sample.x, sample.y),
            pen: sample,
            colour: None,
            level: None,
        };
        self.begin_hold(surface, sample, Contact::Loading(loading));
        self.session.brush.tool = tool;
    }

    fn begin_hold(&mut self, surface: Surface, sample: StrokeSample, contact: Contact) {
        self.release();
        self.session.begin_hold(&self.gpu, surface);
        self.contact = Some(contact);
        self.hold_clock = None;
        self.last_sample_time_ms = None;
        self.note_samples(std::slice::from_ref(&sample));
    }
}

#[cfg(test)]
mod tests;
