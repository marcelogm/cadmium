use gtk::gdk;

use super::brush::BRUSH_WORDING;
use super::panel::{NO_CONTROLS, VisibleControls};
use super::{Press, Tool, UiToolProfile};

pub(super) const BLENDER: UiToolProfile = UiToolProfile {
    name: "blender",
    label: "Blender",
    icon: "cadmium-blender-symbolic",
    tooltip: "Blender (3)",
    keys: &[gdk::Key::_3],
    keypad_keys: &[gdk::Key::KP_3],
    on_tray: Press::Stroke(Tool::Blender),
    on_painting: Press::Stroke(Tool::Blender),
    controls: VisibleControls {
        size: true,
        shape: true,
        ..NO_CONTROLS
    },
    wording: BRUSH_WORDING,
    handling: None,
};
