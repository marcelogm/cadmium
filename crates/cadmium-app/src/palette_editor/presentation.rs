use crate::state::{PaletteEdit, PaletteMode};
use crate::tools::UiTool;

pub(super) const CIRCLE_HINT: &str =
    "Drag colours onto the circle. Drag a point to reorder it, or off the circle to remove it.";
const CIRCLE_CHOOSE_HINT: &str = "Click a point on the circle.";
const MIXING_HINT: &str = "Mix on the sheet, then pick a colour from it.";
const ARMED_HINT: &str = "Click the paint on the sheet.";
const CHOSEN_HINT: &str = "Ready to pin.";
const CIRCLE_MODE: &str = "circle";
const MIXING_MODE: &str = "mixing";
/// The dialog's modes and the label of the toggle of each, in toggle order.
pub(super) const MODES: [(PaletteMode, &str); 2] = [
    (PaletteMode::Circle, "Circle"),
    (PaletteMode::Mixing, "Mixing"),
];

/// What a tool key does inside the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ToolKey {
    /// No tool's key: the key goes on.
    NotATool,
    /// A tool the sheet does not offer: the key stops here, so it never reaches the painting.
    Swallow,
    Choose(UiTool),
}

pub(super) fn pin_button_is_enabled(edit: &PaletteEdit) -> bool {
    edit.chosen.is_some()
}

/// What the painter does next to get a colour to pin.
pub(super) fn chosen_hint(edit: &PaletteEdit) -> &'static str {
    if edit.chosen.is_some() {
        return CHOSEN_HINT;
    }
    match edit.mode {
        PaletteMode::Circle => CIRCLE_CHOOSE_HINT,
        PaletteMode::Mixing if edit.pick_armed => ARMED_HINT,
        PaletteMode::Mixing => MIXING_HINT,
    }
}

/// Whether the column marks the colours that are on the circle.
pub(super) fn shows_circle_markers(mode: PaletteMode) -> bool {
    mode == PaletteMode::Circle
}

pub(super) fn tool_key(picked: Option<UiTool>) -> ToolKey {
    match picked {
        None => ToolKey::NotATool,
        Some(tool) if UiTool::SHEET.contains(&tool) => ToolKey::Choose(tool),
        Some(_) => ToolKey::Swallow,
    }
}

/// The name of the toggle and of the stack page of a mode.
pub(super) fn mode_name(mode: PaletteMode) -> &'static str {
    match mode {
        PaletteMode::Circle => CIRCLE_MODE,
        PaletteMode::Mixing => MIXING_MODE,
    }
}

pub(super) fn mode_named(name: &str) -> Option<PaletteMode> {
    let mut modes = MODES.iter().map(|(mode, _)| *mode);
    modes.find(|mode| mode_name(*mode) == name)
}

#[cfg(test)]
mod tests;
