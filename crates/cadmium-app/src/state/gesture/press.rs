use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;

use super::Contact;
use crate::state::AppState;
use crate::tools::Press;

impl AppState {
    pub fn press(&mut self, surface: Surface, sample: StrokeSample, eraser: bool) -> bool {
        let press = if eraser {
            Press::Stroke(Tool::Rag)
        } else {
            self.ui_tool.profile().press_on(surface)
        };
        let began = match press {
            Press::Pan | Press::PickColour => false,
            Press::Squeeze => {
                let squeezing = Contact::Squeezing {
                    surface,
                    pen: sample,
                    nozzle_at: (sample.x, sample.y),
                    squeezed: false,
                };
                self.begin_hold(surface, sample, squeezing);
                true
            }
            Press::LoadFromPile(tool) => {
                let loading = Contact::Loading {
                    tool,
                    origin: (sample.x, sample.y),
                    pen: sample,
                    colour: None,
                    level: None,
                };
                self.begin_hold(surface, sample, loading);
                self.session.brush.tool = tool;
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
