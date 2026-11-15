use gtk::gdk;

pub use cadmium_engine::stroke::Tool;
use panel::{ToolWording, VisibleControls};

mod blender;
mod brush;
mod eyedropper;
pub(crate) mod keys;
pub(crate) mod knife;
mod moving;
pub(crate) mod panel;
mod pencil;
mod rag;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiTool {
    Move,
    Paint(Tool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Press {
    Pan,
    PickColour,
    Stroke(Tool),
}

pub(crate) struct UiToolProfile {
    name: &'static str,
    label: &'static str,
    pub(crate) icon: &'static str,
    pub(crate) tooltip: &'static str,
    pub(crate) keys: &'static [gdk::Key],
    keypad_keys: &'static [gdk::Key],
    press: Press,
    pub(crate) controls: VisibleControls,
    pub(crate) wording: ToolWording,
    handling: Option<&'static str>,
}

impl UiToolProfile {
    /// What a touch does while this tool is chosen.
    pub(crate) const fn press(&self) -> Press {
        self.press
    }
}

impl Default for UiTool {
    fn default() -> Self {
        UiTool::Paint(Tool::Brush)
    }
}

impl UiTool {
    pub const ALL: [UiTool; 7] = [
        UiTool::Move,
        UiTool::Paint(Tool::Pencil),
        UiTool::Paint(Tool::Brush),
        UiTool::Paint(Tool::Knife),
        UiTool::Paint(Tool::Blender),
        UiTool::Paint(Tool::Rag),
        UiTool::Paint(Tool::Eyedropper),
    ];

    /// The tools the mixing sheet offers: the painting's paint tools, without the pencil.
    pub const SHEET: [UiTool; 5] = [
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
            UiTool::Paint(Tool::Blender) => &blender::BLENDER,
            UiTool::Paint(Tool::Rag) => &rag::RAG,
            UiTool::Paint(Tool::Pencil) => &pencil::PENCIL,
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

/// The toggle that picks `tool` in a tool bar: its icon and tooltip, named by the tool.
pub(crate) fn tool_toggle(tool: UiTool) -> adw::Toggle {
    let profile = tool.profile();
    adw::Toggle::builder()
        .name(tool.name())
        .icon_name(profile.icon)
        .tooltip(profile.tooltip)
        .build()
}

#[cfg(test)]
mod tests;
