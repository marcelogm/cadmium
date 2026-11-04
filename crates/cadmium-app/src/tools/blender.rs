use gtk::gdk;

use super::UiToolProfile;

pub(super) const BLENDER: UiToolProfile = UiToolProfile {
    name: "blender",
    label: "Misturador",
    keys: &[gdk::Key::_3],
    keypad_keys: &[gdk::Key::KP_3],
    handling: None,
};
