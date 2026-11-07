use super::{Tool, UiTool};

pub(crate) const CLEAN_LABEL: &str = "Clean";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToolWording {
    pub(crate) pigments: &'static str,
    pub(crate) loaded_title: &'static str,
    pub(crate) loaded_tooltip: &'static str,
    pub(crate) level_tooltip: &'static str,
    pub(crate) clean: &'static str,
    pub(crate) clean_button: &'static str,
    pub(crate) size_title: &'static str,
    pub(crate) size_subtitle: Option<&'static str>,
    pub(crate) reload_hint: &'static str,
    pub(crate) clean_each_hint: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VisibleControls {
    pub(crate) pigments: bool,
    pub(crate) loaded: bool,
    pub(crate) size: bool,
    pub(crate) load: bool,
    pub(crate) medium: bool,
    pub(crate) plate_gap: bool,
    pub(crate) shape: bool,
    pub(crate) stroke_loading: bool,
    pub(crate) clean: bool,
    pub(crate) hint: bool,
}

impl VisibleControls {
    pub(crate) fn tool_group(self) -> bool {
        self.size || self.loaded || self.shape
    }
}

pub(super) const NO_CONTROLS: VisibleControls = VisibleControls {
    pigments: false,
    loaded: false,
    size: false,
    load: false,
    medium: false,
    plate_gap: false,
    shape: false,
    stroke_loading: false,
    clean: false,
    hint: false,
};

pub(super) const TUBE_LABEL: &str = "Tube";

const TUBE_CONTROLS: VisibleControls = VisibleControls {
    pigments: true,
    size: true,
    ..NO_CONTROLS
};

const TUBE_WORDING: ToolWording = ToolWording {
    pigments: "Click to pick the tube",
    size_title: "Nozzle",
    size_subtitle: None,
    ..UiTool::Paint(Tool::Brush).profile().wording
};

/// The tool the side panel describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PanelTool {
    Paint(Tool),
    Tube,
}

impl PanelTool {
    /// The painting's tool as the panel shows it; Move shows the tool in hand.
    pub(crate) fn of_painting(painting: UiTool, tool_in_hand: Tool) -> PanelTool {
        match painting {
            UiTool::Paint(tool) => PanelTool::Paint(tool),
            UiTool::Move => PanelTool::Paint(tool_in_hand),
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            PanelTool::Paint(tool) => UiTool::Paint(tool).label(),
            PanelTool::Tube => TUBE_LABEL,
        }
    }

    pub(crate) fn holds_knife(self) -> bool {
        matches!(self, PanelTool::Paint(tool) if tool.profile().holds_on_blade())
    }

    pub(crate) fn wording(self) -> ToolWording {
        match self {
            PanelTool::Paint(tool) => UiTool::Paint(tool).profile().wording,
            PanelTool::Tube => TUBE_WORDING,
        }
    }

    pub(crate) fn controls(self) -> VisibleControls {
        match self {
            PanelTool::Paint(tool) => UiTool::Paint(tool).profile().controls,
            PanelTool::Tube => TUBE_CONTROLS,
        }
    }

    /// How the tool is held, for the tools that need telling.
    pub(crate) fn handling(self) -> Option<&'static str> {
        match self {
            PanelTool::Paint(tool) => UiTool::Paint(tool).handling(),
            PanelTool::Tube => None,
        }
    }
}
