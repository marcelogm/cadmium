use cadmium_engine::surface::Surface;

use super::AppState;
use crate::tools::panel::PanelTool;
use crate::tools::{Press, TrayTool};

impl AppState {
    pub fn tray_tool(&self) -> TrayTool {
        self.tray_tool
    }

    pub fn set_tray_tool(&mut self, tool: TrayTool) {
        self.tray_tool = tool;
        self.focus = Surface::Tray;
    }

    /// What a touch on `surface` does: each surface answers with its own tool.
    pub(crate) fn press_for(&self, surface: Surface) -> Press {
        match surface {
            Surface::Painting => self.ui_tool.profile().on_painting,
            Surface::Tray => self.tray_tool.press(self.ui_tool),
        }
    }

    /// The tool the side panel and the pigment piles answer to.
    pub(crate) fn focused_tool(&self) -> PanelTool {
        let tool_in_hand = self.session.brush.tool;
        match self.focus {
            Surface::Painting => PanelTool::of_painting(self.ui_tool, tool_in_hand),
            Surface::Tray => self.tray_tool.panel_tool(self.ui_tool, tool_in_hand),
        }
    }
}

#[cfg(test)]
mod tests;
