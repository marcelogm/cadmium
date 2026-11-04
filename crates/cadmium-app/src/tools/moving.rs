use gtk::gdk;

use super::UiToolProfile;

pub(super) const MOVE: UiToolProfile = UiToolProfile {
    name: "move",
    label: "Mover",
    keys: &[gdk::Key::h],
    keypad_keys: &[],
    handling: None,
};
