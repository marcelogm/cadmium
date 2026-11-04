use gtk::gdk;

use super::{Press, Tool, UiToolProfile};

pub(super) const BRUSH: UiToolProfile = UiToolProfile {
    name: "brush",
    label: "Pincel",
    keys: &[gdk::Key::_1],
    keypad_keys: &[gdk::Key::KP_1],
    on_tray: Press::LoadFromPile(Tool::Brush),
    on_painting: Press::Stroke(Tool::Brush),
    handling: None,
};
