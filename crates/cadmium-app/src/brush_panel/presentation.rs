use cadmium_engine::stroke::Tool;

use crate::tools::{KNIFE_HANDLING, UiTool};

pub const CLEAN_LABEL: &str = "Limpo";
const PERCENT: f64 = 100.0;
const MIN_LISTED_SHARE: f32 = 0.01;
pub const EYEDROPPER_HINT: &str = "Clique na tela para pegar a tinta que está nela. \
O botão direito do mouse e o botão inferior da caneta pegam a cor com qualquer ferramenta.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelTool {
    Paint(Tool),
    Tube,
}

impl PanelTool {
    pub fn label(self) -> &'static str {
        match self {
            PanelTool::Paint(tool) => UiTool::Paint(tool).label(),
            PanelTool::Tube => UiTool::Tube.label(),
        }
    }
}

pub fn panel_tool(ui_tool: UiTool, tool_in_hand: Tool) -> PanelTool {
    match ui_tool {
        UiTool::Paint(tool) => PanelTool::Paint(tool),
        UiTool::Move => PanelTool::Paint(tool_in_hand),
        UiTool::Tube => PanelTool::Tube,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolWording {
    pub pigments: &'static str,
    pub loaded_title: &'static str,
    pub loaded_tooltip: &'static str,
    pub level_tooltip: &'static str,
    pub clean: &'static str,
    pub clean_button: &'static str,
    pub size_title: &'static str,
    pub size_subtitle: Option<&'static str>,
    pub reload_hint: &'static str,
    pub clean_each_hint: &'static str,
}

const BRUSH_WORDING: ToolWording = ToolWording {
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

const TUBE_WORDING: ToolWording = ToolWording {
    pigments: "Clique escolhe o tubo",
    size_title: "Bico",
    size_subtitle: None,
    ..BRUSH_WORDING
};

pub fn tool_wording(tool: PanelTool) -> ToolWording {
    match tool {
        PanelTool::Paint(Tool::Knife) => KNIFE_WORDING,
        PanelTool::Paint(_) => BRUSH_WORDING,
        PanelTool::Tube => TUBE_WORDING,
    }
}

pub fn tool_hint(tool: PanelTool) -> &'static str {
    match tool {
        PanelTool::Paint(Tool::Knife) => KNIFE_HANDLING,
        PanelTool::Paint(_) | PanelTool::Tube => EYEDROPPER_HINT,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisibleControls {
    pub pigments: bool,
    pub loaded: bool,
    pub size: bool,
    pub load: bool,
    pub medium: bool,
    pub plate_gap: bool,
    pub shape: bool,
    pub stroke_loading: bool,
    pub clean: bool,
    pub hint: bool,
}

impl VisibleControls {
    pub fn tool_group(self) -> bool {
        self.size || self.loaded || self.shape
    }
}

const NO_CONTROLS: VisibleControls = VisibleControls {
    pigments: false,
    loaded: false,
    size: false,
    load: false,
    medium: false,
    plate_gap: false,
    shape: false,
    stroke_loading: false,
    clean: false,
    hint: false,
};

pub fn visible_controls(tool: PanelTool) -> VisibleControls {
    match tool {
        PanelTool::Paint(Tool::Brush) => VisibleControls {
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
        PanelTool::Paint(Tool::Knife) => VisibleControls {
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
        PanelTool::Paint(Tool::Blender) => VisibleControls {
            size: true,
            shape: true,
            ..NO_CONTROLS
        },
        PanelTool::Paint(Tool::Rag) => VisibleControls {
            size: true,
            ..NO_CONTROLS
        },
        PanelTool::Paint(Tool::Eyedropper) => VisibleControls {
            hint: true,
            ..NO_CONTROLS
        },
        PanelTool::Tube => VisibleControls {
            pigments: true,
            size: true,
            ..NO_CONTROLS
        },
    }
}

pub fn pile_is_highlighted(
    tool: PanelTool,
    slot: usize,
    tube_slot: usize,
    recipe: &[(usize, f32)],
) -> bool {
    match tool {
        PanelTool::Tube => slot == tube_slot,
        PanelTool::Paint(_) => recipe
            .iter()
            .any(|(chosen, parts)| *chosen == slot && *parts > 0.0),
    }
}

pub fn tool_group_description<'a>(
    tool: PanelTool,
    slot_names: &[&'a str],
    tube_slot: usize,
) -> Option<&'a str> {
    match tool {
        PanelTool::Tube => slot_names.get(tube_slot).copied(),
        PanelTool::Paint(_) => None,
    }
}

pub fn describe_recipe(names: &[&str], recipe: &[(usize, f32)]) -> String {
    let mut shares: Vec<(&str, f32)> = recipe
        .iter()
        .filter(|(_, parts)| *parts > 0.0)
        .filter_map(|(slot, parts)| names.get(*slot).map(|name| (*name, *parts)))
        .collect();
    let total: f32 = shares.iter().map(|(_, parts)| parts).sum();
    if total <= 0.0 {
        return CLEAN_LABEL.to_string();
    }
    if let [(name, _)] = shares.as_slice() {
        return name.to_string();
    }
    shares.sort_by(|a, b| b.1.total_cmp(&a.1));
    shares
        .iter()
        .map(|(name, parts)| (name, parts / total))
        .filter(|(_, share)| *share >= MIN_LISTED_SHARE)
        .map(|(name, share)| format!("{name} {:.0}%", f64::from(share) * PERCENT))
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_description_names_a_single_pigment() {
        assert_eq!(describe_recipe(&["Branco", "Azul"], &[(1, 1.0)]), "Azul");
    }

    #[test]
    fn recipe_description_lists_shares_largest_first() {
        assert_eq!(
            describe_recipe(&["Branco", "Azul", "Amarelo"], &[(1, 1.0), (2, 3.0)]),
            "Amarelo 75% · Azul 25%"
        );
    }

    #[test]
    fn empty_recipe_reads_as_a_clean_brush() {
        assert_eq!(describe_recipe(&["Branco"], &[]), CLEAN_LABEL);
        assert_eq!(describe_recipe(&["Branco"], &[(0, 0.0)]), CLEAN_LABEL);
    }

    #[test]
    fn brush_shows_every_control_and_eyedropper_only_the_hint() {
        let brush = visible_controls(PanelTool::Paint(Tool::Brush));
        assert!(brush.pigments && brush.loaded && brush.size && brush.load && brush.medium);
        assert!(brush.shape && brush.stroke_loading && brush.clean && !brush.hint);
        assert!(!brush.plate_gap);
        let eyedropper = visible_controls(PanelTool::Paint(Tool::Eyedropper));
        assert!(eyedropper.hint);
        assert!(!eyedropper.pigments && !eyedropper.loaded && !eyedropper.size);
        assert!(!eyedropper.load && !eyedropper.medium && !eyedropper.plate_gap);
        assert!(!eyedropper.shape && !eyedropper.clean);
        assert!(!eyedropper.stroke_loading);
    }

    #[test]
    fn knife_loads_from_the_palette_and_sets_its_plate_but_has_no_medium_or_shape() {
        let knife = visible_controls(PanelTool::Paint(Tool::Knife));
        assert!(knife.pigments && knife.loaded && knife.size && knife.load);
        assert!(knife.plate_gap && knife.stroke_loading && knife.clean && knife.hint);
        assert!(!knife.medium && !knife.shape);
    }

    #[test]
    fn rag_shows_only_the_size() {
        let shown = visible_controls(PanelTool::Paint(Tool::Rag));
        assert!(shown.size);
        assert!(!shown.pigments && !shown.loaded && !shown.load && !shown.medium);
        assert!(!shown.plate_gap && !shown.shape && !shown.stroke_loading);
        assert!(!shown.clean && !shown.hint);
    }

    #[test]
    fn knife_panel_speaks_of_the_knife_and_how_to_hold_it() {
        let knife = tool_wording(PanelTool::Paint(Tool::Knife));
        assert_eq!(knife.clean_button, "Limpar espátula");
        assert_eq!(knife.loaded_title, "Na espátula");
        assert_eq!(knife.size_title, "Tamanho");
        assert_eq!(tool_hint(PanelTool::Paint(Tool::Knife)), KNIFE_HANDLING);
        assert_eq!(
            tool_wording(PanelTool::Paint(Tool::Brush)).clean_button,
            "Limpar pincel"
        );
        assert_eq!(
            tool_hint(PanelTool::Paint(Tool::Eyedropper)),
            EYEDROPPER_HINT
        );
    }

    #[test]
    fn the_tube_shows_only_pigments_and_nozzle_size() {
        let tube = visible_controls(PanelTool::Tube);
        assert!(tube.pigments && tube.size && tube.tool_group());
        assert!(!tube.loaded && !tube.load && !tube.medium && !tube.plate_gap);
        assert!(!tube.shape && !tube.stroke_loading && !tube.clean && !tube.hint);
    }

    #[test]
    fn the_tube_names_its_size_nozzle_without_a_subtitle() {
        let tube = tool_wording(PanelTool::Tube);
        assert_eq!(tube.size_title, "Bico");
        assert_eq!(tube.size_subtitle, None);
    }

    #[test]
    fn the_tube_asks_for_a_pile_to_squeeze() {
        assert_eq!(
            tool_wording(PanelTool::Tube).pigments,
            "Clique escolhe o tubo"
        );
    }

    #[test]
    fn moving_keeps_the_panel_of_the_tool_in_hand() {
        assert_eq!(
            panel_tool(UiTool::Move, Tool::Knife),
            PanelTool::Paint(Tool::Knife)
        );
        assert_eq!(panel_tool(UiTool::Tube, Tool::Knife), PanelTool::Tube);
        assert_eq!(
            panel_tool(UiTool::Paint(Tool::Rag), Tool::Brush),
            PanelTool::Paint(Tool::Rag)
        );
    }

    #[test]
    fn the_tube_highlights_its_own_pile_not_the_recipe() {
        let recipe = [(0, 1.0), (2, 0.5)];
        assert!(pile_is_highlighted(PanelTool::Tube, 1, 1, &recipe));
        assert!(!pile_is_highlighted(PanelTool::Tube, 0, 1, &recipe));
        let brush = PanelTool::Paint(Tool::Brush);
        assert!(pile_is_highlighted(brush, 2, 1, &recipe));
        assert!(!pile_is_highlighted(brush, 1, 1, &recipe));
    }

    #[test]
    fn the_tube_group_names_the_chosen_pigment() {
        let names = ["Branco", "Azul"];
        assert_eq!(
            tool_group_description(PanelTool::Tube, &names, 1),
            Some("Azul")
        );
        assert_eq!(
            tool_group_description(PanelTool::Paint(Tool::Brush), &names, 1),
            None
        );
    }
}
