use gtk::gdk;

use super::brush::BRUSH_WORDING;
use super::panel::{NO_CONTROLS, ToolWording, VisibleControls};
use super::{Press, Tool, UiToolProfile};

pub(crate) const KNIFE_HANDLING: &str = "Laid flat (pen tilted) it lays slabs; upright it scrapes and carves a groove; \
light pressure leaves a thicker slab";

pub(super) const KNIFE: UiToolProfile = UiToolProfile {
    name: "knife",
    label: "Knife",
    icon: "cadmium-knife-symbolic",
    tooltip: "Knife (2): flat lays slabs, upright scrapes and carves a groove; light pressure leaves a thicker slab",
    keys: &[gdk::Key::_2],
    keypad_keys: &[gdk::Key::KP_2],
    on_tray: Press::LoadFromPile(Tool::Knife),
    on_painting: Press::Stroke(Tool::Knife),
    controls: VisibleControls {
        pigments: true,
        loaded: true,
        size: true,
        load: true,
        plate_gap: true,
        stroke_loading: true,
        clean: true,
        hint: true,
        ..NO_CONTROLS
    },
    wording: KNIFE_WORDING,
    handling: Some(KNIFE_HANDLING),
};

const KNIFE_WORDING: ToolWording = ToolWording {
    pigments: "Click loads a clean knife, click again to pick up more; Ctrl+click mixes in a little",
    loaded_title: "On the Knife",
    loaded_tooltip: "Left: mix on the palette · right: paint on the knife",
    level_tooltip: "Paint on the knife",
    clean: "Clean",
    clean_button: "Clean Knife",
    size_subtitle: Some("Blade length"),
    reload_hint: "Tops the knife back up to the load",
    clean_each_hint: "Starts every stroke with a clean, reloaded knife",
    ..BRUSH_WORDING
};
