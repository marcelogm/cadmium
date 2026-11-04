use gtk::gdk;

use super::UiToolProfile;

pub(super) const BRUSH: UiToolProfile = UiToolProfile {
    name: "brush",
    label: "Pincel",
    keys: &[gdk::Key::_1],
    keypad_keys: &[gdk::Key::KP_1],
    handling: None,
};
