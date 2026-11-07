use gtk::gdk;

use super::panel::{PanelTool, TUBE_LABEL};
use super::{Press, Tool, UiTool, mixing_knife};

/// What the pen does on the tray, chosen in the tray's own header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrayTool {
    /// Loads the painting tool with the paint under the pen.
    #[default]
    Load,
    /// Mixes paint on the tray with the face of a wide blade.
    MixingKnife,
    Blender,
    Rag,
    Tube,
}

pub(crate) struct TrayToolProfile {
    name: &'static str,
    label: &'static str,
    icon: &'static str,
    pub(crate) tooltip: &'static str,
    key: gdk::Key,
}

const LOAD: TrayToolProfile = TrayToolProfile {
    name: "load",
    label: "Load",
    icon: "cadmium-brush-symbolic",
    tooltip: "Load (Shift+1): touch the tray to load the brush or knife with the paint under it",
    key: gdk::Key::_1,
};
const MIXING_KNIFE: TrayToolProfile = TrayToolProfile {
    name: "tray-mixing-knife",
    label: mixing_knife::MIXING_KNIFE.label,
    icon: mixing_knife::MIXING_KNIFE.icon,
    tooltip: mixing_knife::MIXING_KNIFE.tooltip,
    key: gdk::Key::_2,
};
const BLENDER: TrayToolProfile = TrayToolProfile {
    name: "tray-blender",
    label: "Blender",
    icon: "cadmium-blender-symbolic",
    tooltip: "Blender (Shift+4): blends the paint on the tray",
    key: gdk::Key::_4,
};
const RAG: TrayToolProfile = TrayToolProfile {
    name: "tray-rag",
    label: "Rag",
    icon: "cadmium-rag-symbolic",
    tooltip: "Rag (Shift+5): wipes paint off the tray",
    key: gdk::Key::_5,
};
const TUBE: TrayToolProfile = TrayToolProfile {
    name: "tray-tube",
    label: TUBE_LABEL,
    icon: "cadmium-tube-symbolic",
    tooltip: "Tube (Shift+6): squeezes pure pigment onto the tray",
    key: gdk::Key::_6,
};

impl TrayTool {
    pub const ALL: [TrayTool; 5] = [
        TrayTool::Load,
        TrayTool::MixingKnife,
        TrayTool::Blender,
        TrayTool::Rag,
        TrayTool::Tube,
    ];

    pub(crate) const fn profile(self) -> &'static TrayToolProfile {
        match self {
            TrayTool::Load => &LOAD,
            TrayTool::MixingKnife => &MIXING_KNIFE,
            TrayTool::Blender => &BLENDER,
            TrayTool::Rag => &RAG,
            TrayTool::Tube => &TUBE,
        }
    }

    pub fn name(self) -> &'static str {
        self.profile().name
    }

    pub fn from_name(name: &str) -> Option<TrayTool> {
        TrayTool::ALL.into_iter().find(|tool| tool.name() == name)
    }

    /// The tool's fixed name; `label_for` adds the painting tool Load acts on.
    pub fn label(self) -> &'static str {
        self.profile().label
    }

    pub(crate) fn key(self) -> gdk::Key {
        self.profile().key
    }

    /// What a touch on the tray does while `painting` is the painting's tool.
    pub(crate) fn press(self, painting: UiTool) -> Press {
        match self {
            TrayTool::Load => match loaded_tool(painting) {
                Some(tool) => Press::LoadFromPile(tool),
                None => Press::Nothing,
            },
            TrayTool::MixingKnife => Press::Stroke(Tool::MixingKnife),
            TrayTool::Blender => Press::Stroke(Tool::Blender),
            TrayTool::Rag => Press::Stroke(Tool::Rag),
            TrayTool::Tube => Press::Squeeze,
        }
    }

    pub fn is_enabled(self, painting: UiTool) -> bool {
        self != TrayTool::Load || loaded_tool(painting).is_some()
    }

    /// The tooltip while `painting` is the painting's tool; a disabled Load says why.
    pub fn tooltip_for(self, painting: UiTool) -> String {
        if self.is_enabled(painting) {
            return self.profile().tooltip.to_string();
        }
        let key = self.key().name().unwrap_or_default();
        format!(
            "{} (Shift+{key}): pick the brush or the knife to load it from the tray",
            self.profile().label
        )
    }

    pub fn label_for(self, painting: UiTool) -> String {
        match (self, loaded_tool(painting)) {
            (TrayTool::Load, Some(tool)) => format!("Load {}", UiTool::Paint(tool).label()),
            _ => self.profile().label.to_string(),
        }
    }

    pub fn icon_for(self, painting: UiTool) -> &'static str {
        match (self, loaded_tool(painting)) {
            (TrayTool::Load, Some(tool)) => UiTool::Paint(tool).profile().icon,
            _ => self.profile().icon,
        }
    }

    /// The tool the side panel describes while this one is in use.
    pub(crate) fn panel_tool(self, painting: UiTool, tool_in_hand: Tool) -> PanelTool {
        match self {
            TrayTool::Load => PanelTool::of_painting(painting, tool_in_hand),
            TrayTool::MixingKnife => PanelTool::Paint(Tool::MixingKnife),
            TrayTool::Blender => PanelTool::Paint(Tool::Blender),
            TrayTool::Rag => PanelTool::Paint(Tool::Rag),
            TrayTool::Tube => PanelTool::Tube,
        }
    }
}

fn loaded_tool(painting: UiTool) -> Option<Tool> {
    match painting {
        UiTool::Paint(tool) if tool.profile().holder.is_some() => Some(tool),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
