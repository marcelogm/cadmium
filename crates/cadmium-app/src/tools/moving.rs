use gtk::gdk;

use super::brush::BRUSH_WORDING;
use super::panel::NO_CONTROLS;
use super::{Press, UiToolProfile};

pub(super) const MOVE: UiToolProfile = UiToolProfile {
    name: "move",
    label: "Move",
    icon: "cadmium-move-symbolic",
    tooltip: "Move (H)",
    keys: &[gdk::Key::h],
    keypad_keys: &[],
    on_tray: Press::Pan,
    on_painting: Press::Pan,
    controls: NO_CONTROLS,
    wording: BRUSH_WORDING,
    handling: None,
};
