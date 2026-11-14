use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;

use crate::state::AppState;
use crate::tools::Press;

impl AppState {
    /// A touch on `surface`, which also brings that surface's tool to the side panel.
    pub fn press(&mut self, surface: Surface, sample: StrokeSample, eraser: bool) -> bool {
        self.focus = surface;
        let press = if eraser {
            Press::Stroke(Tool::Rag)
        } else {
            self.press_for(surface)
        };
        let began = match press {
            Press::Pan | Press::Nothing | Press::PickColour => false,
            Press::Stroke(tool) => self.begin_stroke_on(surface, tool, sample),
        };
        self.wake();
        began
    }
}

#[cfg(test)]
mod tests;
