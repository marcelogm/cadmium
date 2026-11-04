use gtk::gdk;

use super::{KNIFE_HANDLING, UiToolProfile};

pub(super) const KNIFE: UiToolProfile = UiToolProfile {
    name: "knife",
    label: "Espátula",
    keys: &[gdk::Key::_2],
    keypad_keys: &[gdk::Key::KP_2],
    handling: Some(KNIFE_HANDLING),
};
