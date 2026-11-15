use cadmium_engine::engine::PaintEngine;
use cadmium_engine::palette::{ColourRef, Recipe, insertion_index, nearest_position};
use cadmium_engine::session::ClearOutcome;

use super::palette::bounded_darken;
use super::{AppState, ColourClick, EaselId};
use crate::tools::{Tool, UiTool};

/// What the Palette dialog shows: the circle to arrange, or the sheet to mix on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaletteMode {
    #[default]
    Circle,
    Mixing,
}

/// The Palette dialog's own state; it starts afresh each time the dialog opens.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaletteEdit {
    pub mode: PaletteMode,
    /// The colour waiting to be pinned to My Colours.
    pub chosen: Option<Recipe>,
    /// Whether the next touch on the sheet chooses the colour under it instead of painting.
    pub pick_armed: bool,
    pub darken: f32,
}

impl AppState {
    /// Opens the palette for editing: the sheet takes the painting's brush and Lab, but keeps
    /// its own paint and colour. Neither the painting nor its tool change.
    pub fn begin_palette_edit(&mut self) {
        self.release(EaselId::Sheet);
        self.palette_edit = PaletteEdit::default();
        self.editing_palette = true;
        self.lend_brush_to_sheet();
        let tool = self.painting.ui_tool;
        let tool = if UiTool::SHEET.contains(&tool) {
            tool
        } else {
            UiTool::Paint(Tool::Brush)
        };
        self.sheet.set_ui_tool(tool);
        self.wake();
    }

    fn lend_brush_to_sheet(&mut self) {
        let painting = &self.painting;
        let lent = &painting.session.brush;
        let brush = &mut self.sheet.session.brush;
        brush.head = lent.head;
        brush.hair = lent.hair;
        brush.geometry = lent.geometry;
        brush.load = lent.load;
        brush.medium = lent.medium;
        self.sheet.session.resize_tip();
        self.sheet.reload_each_stroke = painting.reload_each_stroke;
        self.sheet.clean_each_stroke = painting.clean_each_stroke;
        *self.sheet.session.engine.params_mut() = painting.session.engine.params().clone();
        self.sheet.session.canvas.dirty.mark_all();
    }

    pub fn end_palette_edit(&mut self) {
        self.release(EaselId::Sheet);
        self.palette_edit = PaletteEdit::default();
        self.editing_palette = false;
        self.wake();
    }

    pub fn is_editing_palette(&self) -> bool {
        self.editing_palette
    }

    /// The easel undo and redo go to; `None` while the dialog arranges the circle.
    pub fn history_easel(&self) -> Option<EaselId> {
        if !self.editing_palette {
            return Some(EaselId::Painting);
        }
        match self.palette_edit.mode {
            PaletteMode::Circle => None,
            PaletteMode::Mixing => Some(EaselId::Sheet),
        }
    }

    pub fn set_palette_mode(&mut self, mode: PaletteMode) {
        self.release(EaselId::Sheet);
        self.palette_edit.mode = mode;
        self.palette_edit.chosen = None;
        self.palette_edit.pick_armed = false;
        self.wake();
    }

    /// Keeps the darkening within `0..=MAX_DARKEN`; a value that is not a number is ignored.
    pub fn set_palette_darken(&mut self, darken: f32) {
        if let Some(darken) = bounded_darken(darken) {
            self.palette_edit.darken = darken;
        }
    }

    /// Chooses the colour at a point of the circle, darkened by the dialog's darkening.
    pub fn choose_on_circle(&mut self, turn: f32, radius: f32) {
        self.palette_edit.chosen = self.circle_recipe_at(turn, radius, self.palette_edit.darken);
    }

    pub fn arm_sheet_pick(&mut self) {
        self.palette_edit.pick_armed = true;
    }

    pub(super) fn sheet_pick_is_armed(&self, id: EaselId) -> bool {
        id == EaselId::Sheet && self.palette_edit.pick_armed
    }

    /// Chooses the colour of the sheet's paint under `(x, y)`, leaving the tool as it is, and
    /// disarms the pick; true when there was paint there.
    pub(super) fn choose_from_sheet(&mut self, x: f32, y: f32) -> bool {
        self.release(EaselId::Sheet);
        let gpu = self.gpu.clone();
        self.palette_edit.chosen = self.sheet.colour_at(&gpu, x, y);
        self.palette_edit.pick_armed = false;
        self.wake();
        self.palette_edit.chosen.is_some()
    }

    /// Pins the chosen colour to My Colours and returns its index; `None`, keeping the choice,
    /// when nothing is chosen or My Colours is full.
    pub fn pin_chosen(&mut self) -> Option<usize> {
        let chosen = self.palette_edit.chosen?;
        let index = self.painting.session.palette.pin(chosen)?;
        self.palette_edit.chosen = None;
        self.palette_changed();
        Some(index)
    }

    /// Takes a colour out of My Colours and off the circle; the tools keep the paint they hold.
    pub fn unpin(&mut self, index: usize) {
        let palette = &mut self.painting.session.palette;
        if index < palette.my_colours.len() {
            palette.unpin(index);
            self.palette_changed();
        }
    }

    /// Puts `colour` on the circle at `turn`; one already there moves instead. A colour the
    /// painting does not hold is ignored.
    pub fn drop_on_circle(&mut self, colour: ColourRef, turn: f32) {
        if self.recipe_of(colour).is_none() {
            return;
        }
        let palette = &mut self.painting.session.palette;
        let before = palette.circle.clone();
        palette.insert_in_circle(insertion_index(turn, before.len()), colour);
        if palette.circle != before {
            self.palette_changed();
        }
    }

    /// Moves the circle colour at `from` to the place nearest `turn` and returns where it is now.
    pub fn drag_circle_colour(&mut self, from: usize, turn: f32) -> usize {
        let palette = &mut self.painting.session.palette;
        let Some(to) = nearest_position(turn, palette.circle.len()) else {
            return from;
        };
        if from >= palette.circle.len() || to == from {
            return from;
        }
        palette.move_in_circle(from, to);
        self.palette_changed();
        to
    }

    pub fn remove_circle_colour(&mut self, position: usize) {
        let palette = &mut self.painting.session.palette;
        if position < palette.circle.len() {
            palette.remove_from_circle(position);
            self.palette_changed();
        }
    }

    /// Picks the sheet's tool; one the sheet does not offer is refused, returning false.
    pub fn set_sheet_tool(&mut self, tool: UiTool) -> bool {
        if !UiTool::SHEET.contains(&tool) {
            return false;
        }
        self.sheet.set_ui_tool(tool);
        true
    }

    /// Loads the sheet's tool from a colour of the palette; one the painting does not hold is
    /// ignored.
    pub fn load_sheet_colour(&mut self, colour: ColourRef, click: ColourClick) {
        let Some(recipe) = self.recipe_of(colour) else {
            return;
        };
        self.release(EaselId::Sheet);
        let gpu = self.gpu.clone();
        self.sheet.load_colour(&gpu, &recipe, click);
        self.wake();
    }

    /// Wipes the wet paint off the sheet.
    pub fn clear_sheet(&mut self) {
        self.release(EaselId::Sheet);
        let gpu = self.gpu.clone();
        if let ClearOutcome::Cleared { .. } = self.sheet.session.clear_wet(&gpu) {
            self.edits.record();
        }
        self.wake();
    }

    fn palette_changed(&mut self) {
        self.edits.record();
        self.wake();
    }
}

#[cfg(test)]
mod tests;
