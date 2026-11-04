use gtk::gdk;

use super::{Press, UiToolProfile};

pub(super) const MOVE: UiToolProfile = UiToolProfile {
    name: "move",
    label: "Mover",
    keys: &[gdk::Key::h],
    keypad_keys: &[],
    on_tray: Press::Pan,
    on_painting: Press::Pan,
    handling: None,
};
