use gtk::gdk;

use super::brush::BRUSH_WORDING;
use super::panel::{NO_CONTROLS, ToolWording, VisibleControls};
use super::{Press, UiToolProfile};

pub(super) const TUBE: UiToolProfile = UiToolProfile {
    name: "tube",
    label: "Tubo",
    icon: "cadmium-tube-symbolic",
    tooltip: "Tubo (6): espreme pigmento puro na bandeja ou na tela; a pressão aperta o tubo",
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
    pigments: "Clique escolhe o tubo",
    size_title: "Bico",
    size_subtitle: None,
    ..BRUSH_WORDING
};
