use gtk::gdk;

pub use cadmium_engine::stroke::Tool;
use panel::{ToolWording, VisibleControls};
pub use tray_tool::TrayTool;

mod blender;
mod brush;
mod eyedropper;
pub(crate) mod keys;
pub(crate) mod knife;
pub(crate) mod mixing_knife;
mod moving;
pub(crate) mod panel;
mod rag;
pub(crate) mod tray_tool;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiTool {
    Move,
    Paint(Tool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Press {
    Pan,
    /// The tip does nothing here; the eraser end still wipes.
    Nothing,
    PickColour,
    Stroke(Tool),
    LoadFromPile(Tool),
    Squeeze,
}

pub(crate) struct UiToolProfile {
    name: &'static str,
    label: &'static str,
    pub(crate) icon: &'static str,
    pub(crate) tooltip: &'static str,
    pub(crate) keys: &'static [gdk::Key],
    keypad_keys: &'static [gdk::Key],
    pub(crate) on_painting: Press,
    pub(crate) controls: VisibleControls,
    pub(crate) wording: ToolWording,
    handling: Option<&'static str>,
}

impl Default for UiTool {
    fn default() -> Self {
        UiTool::Paint(Tool::Brush)
    }
}

impl UiTool {
    pub const ALL: [UiTool; 6] = [
        UiTool::Move,
        UiTool::Paint(Tool::Brush),
        UiTool::Paint(Tool::Knife),
        UiTool::Paint(Tool::Blender),
        UiTool::Paint(Tool::Rag),
        UiTool::Paint(Tool::Eyedropper),
    ];

    pub(crate) const fn profile(self) -> &'static UiToolProfile {
        match self {
            UiTool::Move => &moving::MOVE,
            UiTool::Paint(Tool::Brush) => &brush::BRUSH,
            UiTool::Paint(Tool::Knife) => &knife::KNIFE,
            UiTool::Paint(Tool::MixingKnife) => &mixing_knife::MIXING_KNIFE,
            UiTool::Paint(Tool::Blender) => &blender::BLENDER,
            UiTool::Paint(Tool::Rag) => &rag::RAG,
            UiTool::Paint(Tool::Eyedropper) => &eyedropper::EYEDROPPER,
        }
    }

    pub fn from_name(name: &str) -> Option<UiTool> {
        UiTool::ALL.into_iter().find(|tool| tool.name() == name)
    }

    pub fn name(self) -> &'static str {
        self.profile().name
    }

    pub fn label(self) -> &'static str {
        self.profile().label
    }

    pub fn handling(self) -> Option<&'static str> {
        self.profile().handling
    }
}

#[cfg(test)]
mod tests;
