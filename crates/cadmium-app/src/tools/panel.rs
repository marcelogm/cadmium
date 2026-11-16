use cadmium_engine::tool::Holder;

use super::{Tool, UiTool};

pub(crate) const CLEAN_LABEL: &str = "Clean";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToolWording {
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
    pub(crate) hardness: bool,
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
    hardness: false,
};

/// The painting's tool as the side panel shows it; Move shows the tool in hand.
pub(crate) fn panel_tool(painting: UiTool, tool_in_hand: Tool) -> Tool {
    match painting {
        UiTool::Paint(tool) => tool,
        UiTool::Move => tool_in_hand,
    }
}

/// How the side panel describes a tool.
pub(crate) trait ToolPanel {
    fn label(self) -> &'static str;

    /// Where the tool carries its paint, if it carries any.
    fn holder(self) -> Option<Holder>;

    fn wording(self) -> ToolWording;

    fn controls(self) -> VisibleControls;

    /// How the tool is held, for the tools that need telling.
    fn handling(self) -> Option<&'static str>;
}

impl ToolPanel for Tool {
    fn label(self) -> &'static str {
        UiTool::Paint(self).label()
    }

    fn holder(self) -> Option<Holder> {
        self.profile().holder
    }

    fn wording(self) -> ToolWording {
        UiTool::Paint(self).profile().wording
    }

    fn controls(self) -> VisibleControls {
        UiTool::Paint(self).profile().controls
    }

    fn handling(self) -> Option<&'static str> {
        UiTool::Paint(self).handling()
    }
}
