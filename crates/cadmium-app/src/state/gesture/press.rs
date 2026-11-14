use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;

use crate::state::AppState;
use crate::tools::Press;

impl AppState {
    /// What a touch on `surface` does.
    pub(crate) fn press_for(&self, _surface: Surface) -> Press {
        self.ui_tool.profile().on_painting
    }

    pub fn press(&mut self, surface: Surface, sample: StrokeSample, eraser: bool) -> bool {
        let press = if eraser {
            Press::Stroke(Tool::Rag)
        } else {
            self.press_for(surface)
        };
        let began = match press {
            Press::Pan | Press::PickColour => false,
            Press::Stroke(tool) => {
                self.begin_stroke_on(surface, tool, sample);
                true
            }
        };
        self.wake();
        began
    }
}

#[cfg(test)]
mod tests;
