use gtk::gdk;

use super::brush::BRUSH_WORDING;
use super::panel::{NO_CONTROLS, VisibleControls};
use super::{Press, UiToolProfile};

pub(super) const EYEDROPPER: UiToolProfile = UiToolProfile {
    name: "eyedropper",
    label: "Eyedropper",
    icon: "color-select-symbolic",
    tooltip: "Eyedropper (5)",
    keys: &[gdk::Key::_5],
    keypad_keys: &[gdk::Key::KP_5],
    on_painting: Press::PickColour,
    controls: VisibleControls {
        hint: true,
        ..NO_CONTROLS
    },
    wording: BRUSH_WORDING,
    handling: None,
};
