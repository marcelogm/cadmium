use cadmium_engine::stroke::{StrokeSample, Tool};

use crate::state::{AppState, EaselId};
use crate::tools::Press;

impl AppState {
    /// What a touch of the pen's tip does on the easel `id`.
    pub(crate) fn press_for(&self, id: EaselId) -> Press {
        self.easel(id).ui_tool.profile().press()
    }

    pub fn press(&mut self, id: EaselId, sample: StrokeSample, eraser: bool) -> bool {
        let press = if eraser {
            Press::Stroke(Tool::Rag)
        } else {
            self.press_for(id)
        };
        let began = match press {
            Press::Pan | Press::PickColour => false,
            Press::Stroke(tool) => {
                self.begin_stroke(id, tool, sample);
                true
            }
        };
        self.wake();
        began
    }
}

#[cfg(test)]
mod tests;
