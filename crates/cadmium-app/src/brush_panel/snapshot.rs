use cadmium_engine::canvas::primer_srgb8;
use cadmium_engine::engine::PaintEngine;
use cadmium_engine::palette::{ColourRef, circle_axes};
use cadmium_engine::pigments::{Mixture, Pigment, PigmentDb, mixture_srgb8};
use cadmium_engine::tool::Holder;

use super::presentation::{PLATE_GAP_KEY, RackEntry, WheelKey, highlighted_colour, rack_entry};
use crate::state::{AppState, PaletteView};
use crate::thumbnails::ThumbnailKey;
use crate::tools::Tool;
use crate::tools::panel::{ToolPanel, panel_tool};

pub(super) struct PigmentChoice {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) colour_index: &'static str,
    pub(super) colour: [u8; 3],
}

/// The palette group: the pinned colours, the colour in use, the face on show and the wheel.
pub(super) struct PaletteSnapshot {
    pub(super) my_colours: Vec<[u8; 3]>,
    pub(super) highlighted: Option<ColourRef>,
    pub(super) view: PaletteView,
    pub(super) circle_darken: f32,
    /// Whether the painting has the black that darkening adds.
    pub(super) can_darken: bool,
    pub(super) wheel_key: WheelKey,
}

impl PaletteSnapshot {
    fn of(state: &AppState) -> PaletteSnapshot {
        let session = &state.painting.session;
        let slots = &session.canvas.spec.pigment_slots;
        let palette = &session.palette;
        let colour_of = |recipe| mixture_srgb8(&session.db, slots, recipe);
        PaletteSnapshot {
            my_colours: palette.my_colours.iter().map(colour_of).collect(),
            highlighted: highlighted_colour(palette, state.selected_recipe().as_ref()),
            view: state.palette_view,
            circle_darken: state.circle_darken(),
            can_darken: circle_axes(slots).black.is_some(),
            wheel_key: WheelKey::of(palette, slots, state.circle_darken()),
        }
    }
}

pub(super) struct Snapshot {
    pub(super) tool: Tool,
    pub(super) slots: Vec<String>,
    pub(super) slot_names: Vec<&'static str>,
    pub(super) slot_colours: Vec<[u8; 3]>,
    pub(super) tooltips: Vec<String>,
    pub(super) addable: Vec<PigmentChoice>,
    pub(super) palette: PaletteSnapshot,
    pub(super) recipe: Vec<(usize, f32)>,
    pub(super) held_is_clean: bool,
    pub(super) palette_colour: [u8; 3],
    pub(super) held_colour: [u8; 3],
    pub(super) held_level: f32,
    pub(super) diameter_px: f32,
    pub(super) pencil_hardness: f32,
    pub(super) load: f32,
    pub(super) plate_gap: f32,
    pub(super) medium: f32,
    pub(super) brush_entries: Vec<RackEntry>,
    pub(super) brush_keys: Vec<ThumbnailKey>,
    pub(super) brush_index: Option<usize>,
    pub(super) brush_is_user: bool,
    pub(super) reload_each_stroke: bool,
    pub(super) clean_each_stroke: bool,
}

impl Snapshot {
    pub(super) fn of(state: &AppState) -> Snapshot {
        let session = &state.painting.session;
        let db = &session.db;
        let slots = session.canvas.spec.pigment_slots.clone();
        let pigments: Vec<_> = slots.iter().filter_map(|id| db.get(id)).collect();
        let tool = panel_tool(state.painting.ui_tool, session.brush.tool);
        let primer = primer_srgb8(db, session.canvas.spec.primer);
        let colour_of = |mixture: &Mixture| mixture_colour(db, &slots, primer, mixture);
        let palette_colour = colour_of(&state.palette_mixture());
        let (held_colour, held_level, held_is_clean) = held_paint(state, tool, colour_of);
        let worn = state.worn_library_brush();
        Snapshot {
            tool,
            slot_names: pigments.iter().map(|pigment| pigment.name).collect(),
            slot_colours: slots.iter().map(|id| db.masstone_srgb8(id)).collect(),
            tooltips: pigment_tooltips(&pigments),
            addable: addable_pigments(db, &slots),
            palette: PaletteSnapshot::of(state),
            slots,
            recipe: session.brush.recipe.clone(),
            held_is_clean,
            palette_colour,
            held_colour,
            held_level,
            diameter_px: state.brush_diameter_px(),
            pencil_hardness: session.brush.pencil_hardness,
            load: session.brush.load,
            plate_gap: session.engine.params().get(PLATE_GAP_KEY),
            medium: session.brush.medium,
            brush_entries: rack_entries(state),
            brush_keys: state.brushes.all().map(ThumbnailKey::of).collect(),
            brush_index: worn,
            brush_is_user: worn.is_some_and(|index| state.brushes.is_user(index)),
            reload_each_stroke: state.painting.reload_each_stroke,
            clean_each_stroke: state.painting.clean_each_stroke,
        }
    }
}

fn rack_entries(state: &AppState) -> Vec<RackEntry> {
    state
        .brushes
        .all()
        .enumerate()
        .map(|(index, brush)| rack_entry(brush, state.brushes.is_user(index)))
        .collect()
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
    tool: Tool,
    colour_of: impl Fn(&Mixture) -> [u8; 3],
) -> ([u8; 3], f32, bool) {
    match tool.holder() {
        Some(Holder::Blade) => (
            colour_of(&state.painting.knife_mixture),
            state.knife_level(),
            state.knife_is_clean(),
        ),
        Some(Holder::Reservoir) | None => (
            colour_of(&state.painting.tip_mixture),
            state.reserve_level(),
            state.brush_is_clean(),
        ),
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
