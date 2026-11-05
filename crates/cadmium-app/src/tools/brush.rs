use gtk::gdk;

use super::panel::{CLEAN_LABEL, ToolWording, VisibleControls};
use super::{Press, Tool, UiToolProfile};

pub(super) const BRUSH: UiToolProfile = UiToolProfile {
    name: "brush",
    label: "Brush",
    icon: "cadmium-brush-symbolic",
    tooltip: "Brush (1)",
    keys: &[gdk::Key::_1],
    keypad_keys: &[gdk::Key::KP_1],
    on_tray: Press::LoadFromPile(Tool::Brush),
    on_painting: Press::Stroke(Tool::Brush),
    controls: VisibleControls {
        pigments: true,
        loaded: true,
        size: true,
        load: true,
        medium: true,
        plate_gap: false,
        shape: true,
        stroke_loading: true,
        clean: true,
        hint: false,
    },
    wording: BRUSH_WORDING,
    handling: None,
};

pub(super) const BRUSH_WORDING: ToolWording = ToolWording {
    pigments: "Click loads a clean brush, click again to dip deeper; Ctrl+click mixes in a little",
    loaded_title: "On the Brush",
    loaded_tooltip: "Left: mix on the palette · right: brush tip",
    level_tooltip: "Paint in the brush reservoir",
    clean: CLEAN_LABEL,
    clean_button: "Clean Brush",
    size_title: "Size",
    size_subtitle: None,
    reload_hint: "Refills the reservoir with the palette mix",
    clean_each_hint: "Starts every stroke with a clean, reloaded brush",
};
