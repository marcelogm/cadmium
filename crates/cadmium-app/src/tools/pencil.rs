use gtk::gdk;

use super::brush::BRUSH_WORDING;
use super::panel::{NO_CONTROLS, VisibleControls};
use super::{Press, Tool, UiToolProfile};

pub(super) const PENCIL: UiToolProfile = UiToolProfile {
    name: "pencil",
    label: "Pencil",
    icon: "cadmium-pencil-symbolic",
    tooltip: "Pencil (7 or P): sketches in graphite; the rag or the eraser end rubs it out until paint dries over it",
    keys: &[gdk::Key::_7, gdk::Key::p],
    keypad_keys: &[gdk::Key::KP_7],
    on_painting: Press::Stroke(Tool::Pencil),
    controls: VisibleControls {
        size: true,
        ..NO_CONTROLS
    },
    wording: BRUSH_WORDING,
    handling: None,
};
