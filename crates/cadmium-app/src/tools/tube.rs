use gtk::gdk;

use super::brush::BRUSH_WORDING;
use super::panel::{NO_CONTROLS, ToolWording, VisibleControls};
use super::{Press, UiToolProfile};

pub(super) const TUBE: UiToolProfile = UiToolProfile {
    name: "tube",
    label: "Tube",
    icon: "cadmium-tube-symbolic",
    tooltip: "Tube (6): squeezes pure pigment onto the tray or the canvas; pressure squeezes the tube",
    keys: &[gdk::Key::_6],
    keypad_keys: &[gdk::Key::KP_6],
    on_tray: Press::Squeeze,
    on_painting: Press::Squeeze,
    controls: VisibleControls {
        pigments: true,
        size: true,
        ..NO_CONTROLS
    },
    wording: TUBE_WORDING,
    handling: None,
};

const TUBE_WORDING: ToolWording = ToolWording {
    pigments: "Click to pick the tube",
    size_title: "Nozzle",
    size_subtitle: None,
    ..BRUSH_WORDING
};
