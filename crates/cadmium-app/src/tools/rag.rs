use gtk::gdk;

use super::brush::BRUSH_WORDING;
use super::panel::{NO_CONTROLS, VisibleControls};
use super::{Press, Tool, UiToolProfile};

pub(super) const RAG: UiToolProfile = UiToolProfile {
    name: "rag",
    label: "Rag",
    icon: "cadmium-rag-symbolic",
    tooltip: "Rag (4 or R)",
    keys: &[gdk::Key::_4, gdk::Key::r],
    keypad_keys: &[gdk::Key::KP_4],
    on_painting: Press::Stroke(Tool::Rag),
    controls: VisibleControls {
        size: true,
        ..NO_CONTROLS
    },
    wording: BRUSH_WORDING,
    handling: None,
};
