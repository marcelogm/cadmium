use gtk::gdk;

use super::{Press, Tool, UiToolProfile};

pub(super) const RAG: UiToolProfile = UiToolProfile {
    name: "rag",
    label: "Pano",
    keys: &[gdk::Key::_4, gdk::Key::r],
    keypad_keys: &[gdk::Key::KP_4],
    on_tray: Press::Stroke(Tool::Rag),
    on_painting: Press::Stroke(Tool::Rag),
    handling: None,
};
