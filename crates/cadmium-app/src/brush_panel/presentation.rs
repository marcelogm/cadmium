use cadmium_engine::brush::BrushShape;
use cadmium_engine::params::{ParamSpec, SimParams};
use cadmium_engine::stroke::Tool;
use gtk::gdk;

use crate::sliders::format_param;
use crate::state::PigmentClick;
use crate::tools::UiTool;
use crate::tools::panel::{CLEAN_LABEL, ToolWording, VisibleControls};

const PERCENT: f64 = 100.0;
const MIN_LISTED_SHARE: f32 = 0.01;
pub(super) const MIN_SIZE_PX: f64 = 4.0;
pub(super) const MAX_SIZE_PX: f64 = 500.0;
pub(super) const SIZE_STEP: f64 = 0.01;
pub(super) const PLATE_GAP_KEY: &str = "knife_gap_um";
const PLATE_GAP_UNIT: &str = "µm";
pub(super) const EYEDROPPER_HINT: &str = "Click the canvas to pick up the paint on it. \
The right mouse button and the lower pen button pick a colour with any tool.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelTool {
    Paint(Tool),
    Tube,
}

impl PanelTool {
    fn ui_tool(self) -> UiTool {
        match self {
            PanelTool::Paint(tool) => UiTool::Paint(tool),
            PanelTool::Tube => UiTool::Tube,
        }
    }

    pub(super) fn label(self) -> &'static str {
        self.ui_tool().label()
    }

    pub(super) fn holds_knife(self) -> bool {
        matches!(self, PanelTool::Paint(tool) if tool.profile().holds_on_blade())
    }
}

pub(super) fn panel_tool(ui_tool: UiTool, tool_in_hand: Tool) -> PanelTool {
    match ui_tool {
        UiTool::Paint(tool) => PanelTool::Paint(tool),
        UiTool::Move => PanelTool::Paint(tool_in_hand),
        UiTool::Tube => PanelTool::Tube,
    }
}

pub(super) fn tool_wording(tool: PanelTool) -> ToolWording {
    tool.ui_tool().profile().wording
}

pub(super) fn tool_hint(tool: PanelTool) -> &'static str {
    tool.ui_tool().handling().unwrap_or(EYEDROPPER_HINT)
}

pub(super) fn visible_controls(tool: PanelTool) -> VisibleControls {
    tool.ui_tool().profile().controls
}

pub(super) fn pile_is_highlighted(
    tool: PanelTool,
    slot: usize,
    tube_slot: usize,
    recipe: &[(usize, f32)],
) -> bool {
    match tool {
        PanelTool::Tube => slot == tube_slot,
        PanelTool::Paint(_) => recipe
            .iter()
            .any(|(chosen, parts)| *chosen == slot && *parts > 0.0),
    }
}

pub(super) fn tool_group_description<'a>(
    tool: PanelTool,
    slot_names: &[&'a str],
    tube_slot: usize,
) -> Option<&'a str> {
    match tool {
        PanelTool::Tube => slot_names.get(tube_slot).copied(),
        PanelTool::Paint(_) => None,
    }
}

pub(super) fn describe_recipe(names: &[&str], recipe: &[(usize, f32)]) -> String {
    let mut shares: Vec<(&str, f32)> = recipe
        .iter()
        .filter(|(_, parts)| *parts > 0.0)
        .filter_map(|(slot, parts)| names.get(*slot).map(|name| (*name, *parts)))
        .collect();
    let total: f32 = shares.iter().map(|(_, parts)| parts).sum();
    if total <= 0.0 {
        return CLEAN_LABEL.to_string();
    }
    if let [(name, _)] = shares.as_slice() {
        return name.to_string();
    }
    shares.sort_by(|a, b| b.1.total_cmp(&a.1));
    shares
        .iter()
        .map(|(name, parts)| (name, parts / total))
        .filter(|(_, share)| *share >= MIN_LISTED_SHARE)
        .map(|(name, share)| format!("{name} {:.0}%", f64::from(share) * PERCENT))
        .collect::<Vec<_>>()
        .join(" · ")
}

pub(super) fn plate_gap_spec() -> &'static ParamSpec {
    SimParams::specs()
        .iter()
        .find(|spec| spec.key == PLATE_GAP_KEY)
        .expect("the knife plate gap is a simulation parameter")
}

pub(super) fn plate_gap_text(gap_um: f32) -> String {
    format!(
        "{} {PLATE_GAP_UNIT}",
        format_param(plate_gap_spec(), gap_um)
    )
}

pub(super) const SHAPES: [(BrushShape, &str); 3] = [
    (BrushShape::Round, "Round"),
    (BrushShape::Flat, "Flat"),
    (BrushShape::Fan, "Fan"),
];

pub(super) fn pigment_click(modifiers: gdk::ModifierType) -> PigmentClick {
    if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
        PigmentClick::Mix
    } else if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
        PigmentClick::CleanAndLoad
    } else {
        PigmentClick::Choose
    }
}

pub(super) fn slider_needs_sync(slider_value: f64, diameter_px: f32) -> bool {
    slider_to_size(slider_value) != diameter_px
}

pub(super) fn size_to_slider(diameter_px: f32) -> f64 {
    f64::from(diameter_px).clamp(MIN_SIZE_PX, MAX_SIZE_PX).ln()
}

pub(super) fn slider_to_size(value: f64) -> f32 {
    value.exp().clamp(MIN_SIZE_PX, MAX_SIZE_PX).round() as f32
}

pub(super) fn percent_text(value: f64) -> String {
    format!("{:.0}%", value * PERCENT)
}

#[cfg(test)]
mod tests;
