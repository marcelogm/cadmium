use gtk::gdk;

use super::{Press, UiToolProfile};

pub(super) const TUBE: UiToolProfile = UiToolProfile {
    name: "tube",
    label: "Tubo",
    keys: &[gdk::Key::_6],
    keypad_keys: &[gdk::Key::KP_6],
    on_tray: Press::Squeeze,
    on_painting: Press::Squeeze,
    handling: None,
};
