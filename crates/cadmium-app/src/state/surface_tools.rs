use cadmium_engine::engine::Tip;
use cadmium_engine::stroke::Tool;
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

    /// Whether the side panel shows the tray blender's own tip rather than the brush.
    pub(crate) fn shows_tray_tip(&self) -> bool {
        self.focus == Surface::Tray && self.tray_tool == TrayTool::Blender
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

/// The tip a stroke of `tool` on `surface` works: on the tray, the tools that hold no paint
/// (its blender and rag) work the tray's own tip, so they never soil the painting brush.
pub(crate) fn tip_for(surface: Surface, tool: Tool) -> Tip {
    if surface == Surface::Tray && tool.profile().holder.is_none() {
        Tip::Tray
    } else {
        Tip::Painting
    }
}

#[cfg(test)]
mod tests;
