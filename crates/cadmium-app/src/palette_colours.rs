//! How the palette's colours read on screen, wherever they are shown: which one the tool
//! holds, what a click on one does, and what each paint is called.

use adw::prelude::*;
use cadmium_engine::palette::{ColourRef, Palette, Recipe, paint_recipe};
use cadmium_engine::pigments::{MAX_SLOTS, Pigment};
use gtk::gdk;

use crate::state::ColourClick;

/// How far apart, per paint, two recipes may be and still be the same colour.
const SAME_COLOUR_TOLERANCE: f32 = 1e-3;

/// The paint or pinned colour the selected colour is; a paint comes before a pinned colour.
pub(crate) fn highlighted_colour(
    palette: &Palette,
    selected: Option<&Recipe>,
) -> Option<ColourRef> {
    let selected = selected?;
    let paint = (0..MAX_SLOTS).find(|slot| is_same_colour(&paint_recipe(*slot), selected));
    let pinned = &palette.my_colours;
    let mine = || {
        pinned
            .iter()
            .position(|mine| is_same_colour(mine, selected))
    };
    paint
        .map(ColourRef::Paint)
        .or_else(|| mine().map(ColourRef::Mine))
}

fn is_same_colour(one: &Recipe, other: &Recipe) -> bool {
    let mut shares = one.iter().zip(other);
    shares.all(|(one, other)| (one - other).abs() <= SAME_COLOUR_TOLERANCE)
}

/// A click on a colour loads it; with Ctrl, mixes it in.
pub(crate) fn colour_click(modifiers: gdk::ModifierType) -> ColourClick {
    if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
        ColourClick::MixIn
    } else {
        ColourClick::Load
    }
}

/// The keyboard modifiers held now, as a button's click does not carry them.
pub(crate) fn current_modifiers(widget: &impl IsA<gtk::Widget>) -> gdk::ModifierType {
    widget
        .display()
        .default_seat()
        .and_then(|seat| seat.keyboard())
        .map(|keyboard| keyboard.modifier_state())
        .unwrap_or_else(gdk::ModifierType::empty)
}

/// A paint's name and Colour Index, as its swatch's tooltip says them.
pub(crate) fn pigment_tooltip(pigment: &Pigment) -> String {
    format!("{} ({})", pigment.name, pigment.colour_index)
}

#[cfg(test)]
mod tests;
