use cadmium_engine::gpu::GpuContext;
use cadmium_engine::palette::{
    CirclePoint, ColourRef, MAX_DARKEN, Recipe, circle_axes, circle_recipe, normalised,
};
use cadmium_engine::pigments::MAX_SLOTS;

use super::recipe::{ColourClick, ColourDip, PARTIAL_DIP, plan_colour_load, recipe_from_fractions};
use super::{AppState, Easel, EaselId};

/// Which face of the palette the side panel shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaletteView {
    #[default]
    Colours,
    Circle,
}

/// `darken` within `0..=MAX_DARKEN`; `None` for a value that is not a number.
pub(super) fn bounded_darken(darken: f32) -> Option<f32> {
    darken.is_finite().then(|| darken.clamp(0.0, MAX_DARKEN))
}

impl AppState {
    pub fn slot_count(&self) -> usize {
        self.painting.slot_count()
    }

    /// Adds the paint `id` to the painting and to its sheet, so the two keep the same slots.
    pub fn add_pile(&mut self, id: &str) -> Result<usize, String> {
        let slot = self.painting.session.add_slot(&self.gpu, id)?;
        self.edits.record();
        self.sheet.session.add_slot(&self.gpu, id)?;
        self.click_colour(ColourRef::Paint(slot), ColourClick::Load);
        self.wake();
        Ok(slot)
    }

    /// Loads the tool from a colour of the palette; a colour the painting does not hold is ignored.
    pub fn click_colour(&mut self, colour: ColourRef, click: ColourClick) {
        if let Some(recipe) = self.recipe_of(colour) {
            self.load_colour(&recipe, click);
        }
    }

    /// The shares of a colour of the palette; `None` for a colour the painting does not hold.
    pub(super) fn recipe_of(&self, colour: ColourRef) -> Option<Recipe> {
        if matches!(colour, ColourRef::Paint(slot) if slot >= self.slot_count()) {
            return None;
        }
        self.painting.session.palette.recipe_of(colour)
    }

    /// Loads the tool from a point of the colour circle, darkened by `circle_darken`; an empty
    /// circle and a point that is not a number are ignored.
    pub fn click_circle(&mut self, turn: f32, radius: f32, click: ColourClick) {
        if let Some(recipe) = self.circle_recipe_at(turn, radius, self.circle_darken) {
            self.load_colour(&recipe, click);
        }
    }

    /// As `click_circle`, but reads the brush in the next frame, so a burst of drag events
    /// waits on the GPU once per frame rather than once per event.
    pub fn drag_on_circle(&mut self, turn: f32, radius: f32, click: ColourClick) {
        if let Some(recipe) = self.circle_recipe_at(turn, radius, self.circle_darken) {
            self.release(EaselId::Painting);
            let gpu = self.gpu.clone();
            self.painting.take_colour(&gpu, &recipe, click);
            self.painting.measure_brush_next_frame();
            self.wake();
        }
    }

    pub(super) fn circle_recipe_at(&self, turn: f32, radius: f32, darken: f32) -> Option<Recipe> {
        if !turn.is_finite() || !radius.is_finite() {
            return None;
        }
        let point = CirclePoint {
            turn,
            radius,
            darken,
        };
        let session = &self.painting.session;
        let axes = circle_axes(&session.canvas.spec.pigment_slots);
        circle_recipe(&session.palette, axes, point)
    }

    /// The selected colour as shares of the painting's paints; `None` while nothing is selected.
    pub fn selected_recipe(&self) -> Option<Recipe> {
        self.painting.selected_recipe()
    }

    pub fn circle_darken(&self) -> f32 {
        self.circle_darken
    }

    /// Keeps the darkening within `0..=MAX_DARKEN`; a value that is not a number is ignored.
    pub fn set_circle_darken(&mut self, darken: f32) {
        if let Some(darken) = bounded_darken(darken) {
            self.circle_darken = darken;
        }
    }

    fn load_colour(&mut self, colour: &Recipe, click: ColourClick) {
        self.release(EaselId::Painting);
        let gpu = self.gpu.clone();
        self.painting.load_colour(&gpu, colour, click);
        self.wake();
    }
}

impl Easel {
    pub(super) fn slot_count(&self) -> usize {
        self.session.canvas.spec.pigment_slots.len()
    }

    /// The selected colour as shares of the easel's paints; `None` while nothing is selected.
    pub fn selected_recipe(&self) -> Option<Recipe> {
        let usable_slots = self.slot_count().min(MAX_SLOTS);
        let mut parts = [0.0; MAX_SLOTS];
        let selected = self.session.brush.recipe.iter();
        for &(slot, amount) in selected.filter(|(slot, _)| *slot < usable_slots) {
            parts[slot] += amount.max(0.0);
        }
        normalised(&parts)
    }

    pub fn load_colour(&mut self, gpu: &GpuContext, colour: &Recipe, click: ColourClick) {
        self.take_colour(gpu, colour, click);
        self.measure_brush_once_painted(gpu);
    }

    fn take_colour(&mut self, gpu: &GpuContext, colour: &Recipe, click: ColourClick) {
        self.put_down_stroke_tool(gpu);
        let (recipe, dip) = plan_colour_load(&self.session.brush.recipe, colour, click);
        self.session.brush.recipe = recipe;
        match dip {
            ColourDip::CleanAndLoad => self.clean_and_load(gpu),
            ColourDip::AddOnTop(colour) => {
                let mut added = self.session.brush.clone();
                added.recipe = recipe_from_fractions(&colour);
                let mixture = added.reservoir_mixture(self.slot_count());
                self.dip(gpu, &mixture, PARTIAL_DIP);
            }
        }
    }
}

#[cfg(test)]
mod tests;
