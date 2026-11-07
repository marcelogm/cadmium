use super::knife::KNIFE;
use super::panel::{NO_CONTROLS, ToolWording, VisibleControls};
use super::{Press, UiToolProfile};

pub(crate) const MIXING_KNIFE_HANDLING: &str = "Laid flat (pen tilted) it spreads what it carries; \
upright it scrapes paint up onto its face; each pass mixes a little more";

/// The tray's mixing knife. It is picked from the tray header, never from the
/// tool header, so it has no keys here and does nothing on the painting; the
/// profile only words and shapes the side panel while it is focused.
pub(super) const MIXING_KNIFE: UiToolProfile = UiToolProfile {
    name: "mixing-knife",
    label: "Mixing Knife",
    icon: "cadmium-mixing-knife-symbolic",
    tooltip: "Mixing Knife (Shift+2): mixes paint on the tray",
    keys: &[],
    keypad_keys: &[],
    on_painting: Press::Nothing,
    controls: VisibleControls {
        loaded: true,
        size: true,
        clean: true,
        hint: true,
        ..NO_CONTROLS
    },
    wording: MIXING_KNIFE_WORDING,
    handling: Some(MIXING_KNIFE_HANDLING),
};

const MIXING_KNIFE_WORDING: ToolWording = ToolWording {
    loaded_title: "On the Mixing Knife",
    loaded_tooltip: "Paint on the mixing knife's face",
    level_tooltip: "Paint on the mixing knife",
    clean_button: "Clean Mixing Knife",
    ..KNIFE.wording
};
