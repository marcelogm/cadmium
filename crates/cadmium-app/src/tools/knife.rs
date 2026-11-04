use gtk::gdk;

use super::{KNIFE_HANDLING, Press, Tool, UiToolProfile};

pub(super) const KNIFE: UiToolProfile = UiToolProfile {
    name: "knife",
    label: "Espátula",
    keys: &[gdk::Key::_2],
    keypad_keys: &[gdk::Key::KP_2],
    on_tray: Press::LoadFromPile(Tool::Knife),
    on_painting: Press::Stroke(Tool::Knife),
    handling: Some(KNIFE_HANDLING),
};
