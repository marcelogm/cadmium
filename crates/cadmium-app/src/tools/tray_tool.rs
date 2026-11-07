use gtk::gdk;

use super::{Press, Tool, UiTool};

/// What the pen does on the tray, chosen in the tray's own header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrayTool {
    /// Loads the painting tool with the paint under the pen.
    #[default]
    Load,
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
    label: "Tube",
    icon: "cadmium-tube-symbolic",
    tooltip: "Tube (Shift+6): squeezes pure pigment onto the tray",
    key: gdk::Key::_6,
};

impl TrayTool {
    pub const ALL: [TrayTool; 4] = [
        TrayTool::Load,
        TrayTool::Blender,
        TrayTool::Rag,
        TrayTool::Tube,
    ];

    pub(crate) const fn profile(self) -> &'static TrayToolProfile {
        match self {
            TrayTool::Load => &LOAD,
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

    pub(crate) fn key(self) -> gdk::Key {
        self.profile().key
    }

    /// What a touch on the tray does while `painting` is the painting's tool.
    pub(crate) fn press(self, painting: UiTool) -> Press {
        match self {
            TrayTool::Load => match loaded_tool(painting) {
                Some(tool) => Press::LoadFromPile(tool),
                None => Press::Pan,
            },
            TrayTool::Blender => Press::Stroke(Tool::Blender),
            TrayTool::Rag => Press::Stroke(Tool::Rag),
            TrayTool::Tube => Press::Squeeze,
        }
    }

    pub fn is_enabled(self, painting: UiTool) -> bool {
        self != TrayTool::Load || loaded_tool(painting).is_some()
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
    pub fn panel_tool(self, painting: UiTool) -> UiTool {
        match self {
            TrayTool::Load => painting,
            TrayTool::Blender => UiTool::Paint(Tool::Blender),
            TrayTool::Rag => UiTool::Paint(Tool::Rag),
            TrayTool::Tube => UiTool::Tube,
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
