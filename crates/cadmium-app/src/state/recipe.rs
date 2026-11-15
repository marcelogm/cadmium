use cadmium_engine::palette::Recipe;
use cadmium_engine::pigments::MAX_SLOTS;

const NEGLIGIBLE_PICKED_FRACTION: f32 = 1e-3;
pub(super) const PARTIAL_DIP: f32 = 0.25;

pub(super) fn recipe_from_fractions(fractions: &[f32; MAX_SLOTS]) -> Vec<(usize, f32)> {
    fractions
        .iter()
        .enumerate()
        .filter(|(_, fraction)| **fraction > NEGLIGIBLE_PICKED_FRACTION)
        .map(|(slot, fraction)| (slot, *fraction))
        .collect()
}

pub(super) fn recipe_for_palette(
    recipe: &[(usize, f32)],
    old_palette: &[String],
    new_palette: &[String],
) -> Vec<(usize, f32)> {
    recipe
        .iter()
        .filter_map(|(slot, share)| {
            let pigment = old_palette.get(*slot)?;
            let slot = new_palette.iter().position(|id| id == pigment)?;
            Some((slot, *share))
        })
        .collect()
}

/// What a click on a colour asks for: the colour alone, or a little of it on top of what is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourClick {
    Load,
    MixIn,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ColourDip {
    CleanAndLoad,
    AddOnTop(Recipe),
}

fn selected_total(selected: &[(usize, f32)]) -> f32 {
    selected.iter().map(|(_, parts)| parts.max(0.0)).sum()
}

fn add_parts(recipe: &mut Vec<(usize, f32)>, slot: usize, added: f32) {
    match recipe.iter_mut().find(|(chosen, _)| *chosen == slot) {
        Some((_, parts)) => *parts += added,
        None => recipe.push((slot, added)),
    }
}

/// The selection a click on `colour` leaves and how the tool takes the paint: mixing in adds
/// `PARTIAL_DIP` of what is selected, and loads the colour when nothing is.
pub(super) fn plan_colour_load(
    selected: &[(usize, f32)],
    colour: &Recipe,
    click: ColourClick,
) -> (Vec<(usize, f32)>, ColourDip) {
    let total = selected_total(selected);
    if click == ColourClick::Load || total <= 0.0 {
        return (recipe_from_fractions(colour), ColourDip::CleanAndLoad);
    }
    let mut recipe = selected.to_vec();
    for (slot, share) in recipe_from_fractions(colour) {
        add_parts(&mut recipe, slot, share * PARTIAL_DIP * total);
    }
    (recipe, ColourDip::AddOnTop(*colour))
}

#[cfg(test)]
mod tests;
