use cadmium_engine::palette::{
    CirclePoint, ColourRef, MAX_DARKEN, Recipe, circle_axes, circle_recipe, normalised,
};
use cadmium_engine::pigments::MAX_SLOTS;

use super::AppState;
use super::recipe::{ColourClick, ColourDip, PARTIAL_DIP, plan_colour_load, recipe_from_fractions};

/// Which face of the palette the side panel shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaletteView {
    #[default]
    Colours,
    Circle,
}

impl AppState {
    pub fn slot_count(&self) -> usize {
        self.session.canvas.spec.pigment_slots.len()
    }

    pub fn add_pile(&mut self, id: &str) -> Result<usize, String> {
        let slot = self.session.add_slot(&self.gpu, id)?;
        self.edits.record();
        self.click_colour(ColourRef::Paint(slot), ColourClick::Load);
        self.wake();
        Ok(slot)
    }

    /// Loads the tool from a colour of the palette; a colour the painting does not hold is ignored.
    pub fn click_colour(&mut self, colour: ColourRef, click: ColourClick) {
        if matches!(colour, ColourRef::Paint(slot) if slot >= self.slot_count()) {
            return;
        }
        if let Some(recipe) = self.session.palette.recipe_of(colour) {
            self.load_colour(&recipe, click);
        }
    }

    /// Loads the tool from a point of the colour circle, darkened by `circle_darken`; an empty
    /// circle and a point that is not a number are ignored.
    pub fn click_circle(&mut self, turn: f32, radius: f32, click: ColourClick) {
        if let Some(recipe) = self.circle_recipe_at(turn, radius) {
            self.load_colour(&recipe, click);
        }
    }

    /// As `click_circle`, but reads the brush in the next frame, so a burst of drag events
    /// waits on the GPU once per frame rather than once per event.
    pub fn drag_on_circle(&mut self, turn: f32, radius: f32, click: ColourClick) {
        if let Some(recipe) = self.circle_recipe_at(turn, radius) {
            self.take_colour(&recipe, click);
            self.measure_brush_next_frame();
            self.wake();
        }
    }

    fn circle_recipe_at(&self, turn: f32, radius: f32) -> Option<Recipe> {
        if !turn.is_finite() || !radius.is_finite() {
            return None;
        }
        let point = CirclePoint {
            turn,
            radius,
            darken: self.circle_darken,
        };
        let axes = circle_axes(&self.session.canvas.spec.pigment_slots);
        circle_recipe(&self.session.palette, axes, point)
    }

    /// The selected colour as shares of the painting's paints; `None` while nothing is selected.
    pub fn selected_recipe(&self) -> Option<Recipe> {
        let usable_slots = self.slot_count().min(MAX_SLOTS);
        let mut parts = [0.0; MAX_SLOTS];
        let selected = self.session.brush.recipe.iter();
        for &(slot, amount) in selected.filter(|(slot, _)| *slot < usable_slots) {
            parts[slot] += amount.max(0.0);
        }
        normalised(&parts)
    }

    pub fn circle_darken(&self) -> f32 {
        self.circle_darken
    }

    /// Keeps the darkening within `0..=MAX_DARKEN`; a value that is not a number is ignored.
    pub fn set_circle_darken(&mut self, darken: f32) {
        if darken.is_finite() {
            self.circle_darken = darken.clamp(0.0, MAX_DARKEN);
        }
    }

    fn load_colour(&mut self, colour: &Recipe, click: ColourClick) {
        self.take_colour(colour, click);
        self.measure_brush_once_painted();
        self.wake();
    }

    fn take_colour(&mut self, colour: &Recipe, click: ColourClick) {
        self.put_down_stroke_tool();
        let (recipe, dip) = plan_colour_load(&self.session.brush.recipe, colour, click);
        self.session.brush.recipe = recipe;
        match dip {
            ColourDip::CleanAndLoad => self.clean_and_load(),
            ColourDip::AddOnTop(colour) => {
                let mut added = self.session.brush.clone();
                added.recipe = recipe_from_fractions(&colour);
                let mixture = added.reservoir_mixture(self.slot_count());
                self.dip(&mixture, PARTIAL_DIP);
            }
        }
    }
}

#[cfg(test)]
mod tests;
