use gtk::gdk;

use super::{Press, Tool, UiToolProfile};

pub(super) const BLENDER: UiToolProfile = UiToolProfile {
    name: "blender",
    label: "Misturador",
    keys: &[gdk::Key::_3],
    keypad_keys: &[gdk::Key::KP_3],
    on_tray: Press::Stroke(Tool::Blender),
    on_painting: Press::Stroke(Tool::Blender),
    handling: None,
};
