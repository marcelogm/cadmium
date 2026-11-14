use cadmium_engine::stroke::{StrokeSample, Tool};

use crate::state::AppState;
use crate::tools::Press;

impl AppState {
    pub fn press(&mut self, sample: StrokeSample, eraser: bool) -> bool {
        let press = if eraser {
            Press::Stroke(Tool::Rag)
        } else {
            self.ui_tool.profile().press()
        };
        let began = match press {
            Press::Pan | Press::PickColour => false,
            Press::Stroke(tool) => {
                self.begin_stroke(tool, sample);
                true
            }
        };
        self.wake();
        began
    }
}

#[cfg(test)]
mod tests;
