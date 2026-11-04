use gtk::gdk;

use super::UiToolProfile;

pub(super) const RAG: UiToolProfile = UiToolProfile {
    name: "rag",
    label: "Pano",
    keys: &[gdk::Key::_4, gdk::Key::r],
    keypad_keys: &[gdk::Key::KP_4],
    handling: None,
};
