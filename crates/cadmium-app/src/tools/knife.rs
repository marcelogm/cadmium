use gtk::gdk;

use super::brush::BRUSH_WORDING;
use super::panel::{NO_CONTROLS, ToolWording, VisibleControls};
use super::{KNIFE_HANDLING, Press, Tool, UiToolProfile};

pub(super) const KNIFE: UiToolProfile = UiToolProfile {
    name: "knife",
    label: "Espátula",
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
    pigments: "Clique carrega a espátula limpa, de novo pega mais; Ctrl+clique mistura um pouco",
    loaded_title: "Na espátula",
    loaded_tooltip: "Esquerda: mistura na paleta · direita: tinta na espátula",
    level_tooltip: "Tinta na espátula",
    clean: "Limpa",
    clean_button: "Limpar espátula",
    size_subtitle: Some("Comprimento da lâmina"),
    reload_hint: "Repõe a tinta da espátula até a carga",
    clean_each_hint: "Começa cada traço com a espátula limpa e recarregada",
    ..BRUSH_WORDING
};
