use gtk::gdk;

pub use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;

mod blender;
mod brush;
mod eyedropper;
pub(crate) mod keys;
mod knife;
mod moving;
mod rag;
mod tube;

pub const KNIFE_HANDLING: &str = "Deitada (caneta inclinada) assenta placas; em pé raspa e abre sulco; \
pressão leve deixa a placa mais grossa";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiTool {
    Move,
    Paint(Tool),
    Tube,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Press {
    Pan,
    PickColour,
    Stroke(Tool),
    LoadFromPile(Tool),
    Squeeze,
}

pub(crate) struct UiToolProfile {
    name: &'static str,
    label: &'static str,
    pub(crate) keys: &'static [gdk::Key],
    keypad_keys: &'static [gdk::Key],
    on_tray: Press,
    on_painting: Press,
    handling: Option<&'static str>,
}

impl UiToolProfile {
    pub(crate) fn press_on(&self, surface: Surface) -> Press {
        match surface {
            Surface::Tray => self.on_tray,
            Surface::Painting => self.on_painting,
        }
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
        UiTool::Paint(Tool::Brush),
        UiTool::Paint(Tool::Knife),
        UiTool::Paint(Tool::Blender),
        UiTool::Paint(Tool::Rag),
        UiTool::Paint(Tool::Eyedropper),
        UiTool::Tube,
    ];

    pub(crate) const fn profile(self) -> &'static UiToolProfile {
        match self {
            UiTool::Move => &moving::MOVE,
            UiTool::Paint(Tool::Brush) => &brush::BRUSH,
            UiTool::Paint(Tool::Knife) => &knife::KNIFE,
            UiTool::Paint(Tool::Blender) => &blender::BLENDER,
            UiTool::Paint(Tool::Rag) => &rag::RAG,
            UiTool::Paint(Tool::Eyedropper) => &eyedropper::EYEDROPPER,
            UiTool::Tube => &tube::TUBE,
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
