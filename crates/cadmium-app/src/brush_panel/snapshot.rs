use cadmium_engine::canvas::primer_srgb8;
use cadmium_engine::engine::PaintEngine;
use cadmium_engine::pigments::{Mixture, Pigment, PigmentDb, mixture_srgb8};

use super::presentation::{PLATE_GAP_KEY, PanelTool, panel_tool, worn_brush};
use crate::state::AppState;

pub(super) struct PigmentChoice {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) colour_index: &'static str,
    pub(super) colour: [u8; 3],
}

pub(super) struct Snapshot {
    pub(super) tool: PanelTool,
    pub(super) tube_slot: usize,
    pub(super) slots: Vec<String>,
    pub(super) slot_names: Vec<&'static str>,
    pub(super) slot_colours: Vec<[u8; 3]>,
    pub(super) tooltips: Vec<String>,
    pub(super) addable: Vec<PigmentChoice>,
    pub(super) recipe: Vec<(usize, f32)>,
    pub(super) held_is_clean: bool,
    pub(super) palette_colour: [u8; 3],
    pub(super) held_colour: [u8; 3],
    pub(super) held_level: f32,
    pub(super) diameter_px: f32,
    pub(super) load: f32,
    pub(super) plate_gap: f32,
    pub(super) medium: f32,
    pub(super) brush_names: Vec<String>,
    pub(super) brush_index: Option<usize>,
    pub(super) brush_is_user: bool,
    pub(super) reload_each_stroke: bool,
    pub(super) clean_each_stroke: bool,
}

impl Snapshot {
    pub(super) fn of(state: &AppState) -> Snapshot {
        let session = &state.session;
        let db = &session.db;
        let slots = session.canvas.spec.pigment_slots.clone();
        let pigments: Vec<_> = slots.iter().filter_map(|id| db.get(id)).collect();
        let tool = panel_tool(state.ui_tool, session.brush.tool);
        let primer = primer_srgb8(db, session.canvas.spec.primer);
        let colour_of = |mixture: &Mixture| mixture_colour(db, &slots, primer, mixture);
        let palette_colour = colour_of(&state.palette_mixture());
        let (held_colour, held_level, held_is_clean) = held_paint(state, tool, colour_of);
        let worn = worn_brush(state.brushes.all(), &session.brush);
        Snapshot {
            tool,
            tube_slot: state.tube_slot,
            slot_names: pigments.iter().map(|pigment| pigment.name).collect(),
            slot_colours: slots.iter().map(|id| db.masstone_srgb8(id)).collect(),
            tooltips: pigment_tooltips(&pigments),
            addable: addable_pigments(db, &slots),
            slots,
            recipe: session.brush.recipe.clone(),
            held_is_clean,
            palette_colour,
            held_colour,
            held_level,
            diameter_px: session.brush.geometry.max_diameter_px,
            load: session.brush.load,
            plate_gap: session.engine.params().get(PLATE_GAP_KEY),
            medium: session.brush.medium,
            brush_names: state
                .brushes
                .all()
                .map(|brush| brush.name.clone())
                .collect(),
            brush_index: worn,
            brush_is_user: worn.is_some_and(|index| state.brushes.is_user(index)),
            reload_each_stroke: state.reload_each_stroke,
            clean_each_stroke: state.clean_each_stroke,
        }
    }
}

fn mixture_colour(db: &PigmentDb, slots: &[String], primer: [u8; 3], mixture: &Mixture) -> [u8; 3] {
    if mixture.is_empty() {
        primer
    } else {
        mixture_srgb8(db, slots, &mixture.fractions)
    }
}

fn held_paint(
    state: &AppState,
    tool: PanelTool,
    colour_of: impl Fn(&Mixture) -> [u8; 3],
) -> ([u8; 3], f32, bool) {
    if tool.holds_knife() {
        (
            colour_of(&state.knife_mixture),
            state.knife_level(),
            state.knife_is_clean(),
        )
    } else {
        (
            colour_of(&state.tip_mixture),
            state.reserve_level(),
            state.brush_is_clean(),
        )
    }
}

fn pigment_tooltips(pigments: &[&Pigment]) -> Vec<String> {
    pigments
        .iter()
        .map(|pigment| format!("{} ({})", pigment.name, pigment.colour_index))
        .collect()
}

fn addable_pigments(db: &PigmentDb, slots: &[String]) -> Vec<PigmentChoice> {
    db.all()
        .iter()
        .filter(|pigment| !slots.iter().any(|id| id == pigment.id))
        .map(|pigment| PigmentChoice {
            id: pigment.id,
            name: pigment.name,
            colour_index: pigment.colour_index,
            colour: db.masstone_srgb8(pigment.id),
        })
        .collect()
}
