//! What the editor's example strokes are painted with: the painting's own paint (C4).

use cadmium_engine::engine::PaintEngine;
use cadmium_engine::pigments::MAX_SLOTS;
use cadmium_studio::{DEMO_PIGMENT, StudioPaint};

use super::AppState;
use crate::presets::WHITES;

/// How much a brush that would show nothing is loaded with, for the examples.
const FALLBACK_LOAD: f32 = 1.0;

impl AppState {
    /// The painting's canvas and paint (C4). A brush that would show nothing — empty, or
    /// holding only whites, which vanish on a white primer — is shown loaded with the
    /// studio's demo pigment instead, like the rack's thumbnails.
    pub fn studio_paint(&self) -> StudioPaint {
        let spec = &self.session.canvas.spec;
        let brush = &self.session.brush;
        let mut paint = StudioPaint {
            slots: spec.pigment_slots.clone(),
            primer: spec.primer,
            weave: spec.weave,
            recipe: brush.recipe.clone(),
            load: brush.load,
            medium: brush.medium,
            params: self.session.engine.params().clone(),
        };
        if shows_nothing(&paint) {
            load_demo_pigment(&mut paint);
        }
        paint
    }
}

/// Nothing on the brush, or only whites (a slot outside the palette counts as nothing).
fn shows_nothing(paint: &StudioPaint) -> bool {
    let is_white = |slot: usize| {
        paint
            .slots
            .get(slot)
            .is_none_or(|id| WHITES.contains(&id.as_str()))
    };
    paint.load <= 0.0
        || paint
            .recipe
            .iter()
            .filter(|(_, share)| *share > 0.0)
            .all(|(slot, _)| is_white(*slot))
}

/// Loads the demo pigment fully: the painting's own slot of it when it has one, else an
/// extra slot of the example canvas when there is room, else (a full palette without it)
/// the first slot that is not a white.
fn load_demo_pigment(paint: &mut StudioPaint) {
    let slot = demo_slot(&mut paint.slots);
    paint.recipe = vec![(slot, 1.0)];
    paint.load = FALLBACK_LOAD;
}

fn demo_slot(slots: &mut Vec<String>) -> usize {
    if let Some(slot) = slots.iter().position(|id| id == DEMO_PIGMENT) {
        return slot;
    }
    if slots.len() < MAX_SLOTS {
        slots.push(DEMO_PIGMENT.to_string());
        return slots.len() - 1;
    }
    slots
        .iter()
        .position(|id| !WHITES.contains(&id.as_str()))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
