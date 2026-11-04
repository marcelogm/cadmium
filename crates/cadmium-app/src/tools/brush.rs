use gtk::gdk;

use super::panel::{CLEAN_LABEL, ToolWording, VisibleControls};
use super::{Press, Tool, UiToolProfile};

pub(super) const BRUSH: UiToolProfile = UiToolProfile {
    name: "brush",
    label: "Pincel",
    icon: "cadmium-brush-symbolic",
    tooltip: "Pincel (1)",
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
    pigments: "Clique carrega o pincel limpo, de novo mergulha mais; Ctrl+clique mistura um pouco",
    loaded_title: "No pincel",
    loaded_tooltip: "Esquerda: mistura na paleta · direita: ponta do pincel",
    level_tooltip: "Tinta na reserva do pincel",
    clean: CLEAN_LABEL,
    clean_button: "Limpar pincel",
    size_title: "Tamanho",
    size_subtitle: None,
    reload_hint: "Repõe a reserva com a mistura da paleta",
    clean_each_hint: "Começa cada traço com o pincel limpo e recarregado",
};
