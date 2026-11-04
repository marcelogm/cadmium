use gtk::gdk;

use super::UiToolProfile;

pub(super) const TUBE: UiToolProfile = UiToolProfile {
    name: "tube",
    label: "Tubo",
    keys: &[gdk::Key::_6],
    keypad_keys: &[gdk::Key::KP_6],
    handling: None,
};
