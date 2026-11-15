use cadmium_engine::palette::{ColourRef, circle_axes};
use cadmium_engine::pigments::mixture_srgb8;

use crate::brush_panel::{WheelKey, highlighted_colour};
use crate::state::{AppState, PaletteEdit, PaletteMode};
use crate::tools::UiTool;

/// What the dialog shows, read from the state at once.
pub(super) struct Snapshot {
    pub(super) edit: PaletteEdit,
    pub(super) chosen_colour: Option<[u8; 3]>,
    pub(super) paints: Vec<[u8; 3]>,
    pub(super) paint_tooltips: Vec<String>,
    pub(super) my_colours: Vec<[u8; 3]>,
    /// The colours on the circle, in circle order.
    pub(super) on_circle: Vec<ColourRef>,
    /// The colour the sheet's tool holds, while the sheet shows.
    pub(super) highlighted: Option<ColourRef>,
    pub(super) sheet_tool: UiTool,
    /// Whether the painting has the black that darkening adds.
    pub(super) can_darken: bool,
    pub(super) wheel_key: WheelKey,
}

impl Snapshot {
    pub(super) fn of(state: &AppState) -> Snapshot {
        let session = &state.painting.session;
        let db = &session.db;
        let slots = &session.canvas.spec.pigment_slots;
        let palette = &session.palette;
        let edit = state.palette_edit.clone();
        let colour_of = |recipe| mixture_srgb8(db, slots, recipe);
        let highlighted = match edit.mode {
            PaletteMode::Circle => None,
            PaletteMode::Mixing => {
                highlighted_colour(palette, state.sheet.selected_recipe().as_ref())
            }
        };
        Snapshot {
            chosen_colour: edit.chosen.as_ref().map(colour_of),
            paints: slots.iter().map(|id| db.masstone_srgb8(id)).collect(),
            paint_tooltips: slots
                .iter()
                .map(|id| match db.get(id) {
                    Some(pigment) => format!("{} ({})", pigment.name, pigment.colour_index),
                    None => id.clone(),
                })
                .collect(),
            my_colours: palette.my_colours.iter().map(colour_of).collect(),
            on_circle: palette.circle.clone(),
            highlighted,
            sheet_tool: state.sheet.ui_tool,
            can_darken: circle_axes(slots).black.is_some(),
            wheel_key: WheelKey::of(palette, slots, edit.darken),
            edit,
        }
    }
}
