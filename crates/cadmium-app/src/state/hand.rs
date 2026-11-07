use cadmium_engine::brush::BrushSpec;
use cadmium_engine::surface::Surface;

use gtk::glib;

use super::AppState;
use crate::LOG_DOMAIN;
use crate::tools::UiTool;

impl AppState {
    pub fn holds_knife(&self) -> bool {
        self.session.brush.tool.profile().holds_on_blade()
    }

    pub(super) fn put_down_stroke_tool(&mut self) {
        self.release();
        self.session.brush.tool = self.resting_tool;
    }

    pub fn set_brush_size(&mut self, diameter_px: f32) {
        self.release();
        self.session.brush.geometry.max_diameter_px = diameter_px;
        self.session.resize_tip();
    }

    /// Wears a brush that is not picked from the library, forgetting the picked one.
    pub fn wear_brush(&mut self, brush: &BrushSpec) {
        self.worn_brush_index = None;
        self.release();
        self.session.brush.wear(brush);
        self.session.resize_tip();
    }

    pub fn set_load(&mut self, load: f32) {
        self.session.brush.load = load;
    }

    pub fn set_medium(&mut self, medium: f32) {
        self.session.brush.medium = medium;
    }

    /// Picks the painting's tool; one the tool header doesn't offer is ignored.
    pub fn set_ui_tool(&mut self, tool: UiTool) {
        if !UiTool::ALL.contains(&tool) {
            glib::g_warning!(
                LOG_DOMAIN,
                "{tool:?} is not a painting tool; keeping the one in hand"
            );
            return;
        }
        self.focus = Surface::Painting;
        self.ui_tool = tool;
        let UiTool::Paint(paint_tool) = tool else {
            return;
        };
        self.resting_tool = paint_tool;
        if !self.is_stroking() {
            self.session.brush.tool = paint_tool;
        }
    }
}

#[cfg(test)]
mod tests;
