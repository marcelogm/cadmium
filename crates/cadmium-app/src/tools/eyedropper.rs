use gtk::gdk;

use super::UiToolProfile;

pub(super) const EYEDROPPER: UiToolProfile = UiToolProfile {
    name: "eyedropper",
    label: "Conta-gotas",
    keys: &[gdk::Key::_5],
    keypad_keys: &[gdk::Key::KP_5],
    handling: None,
};
