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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PigmentClick {
    Choose,
    Mix,
    CleanAndLoad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PigmentDip {
    CleanAndLoad,
    DipAgain,
    AddOnTop(usize),
}

fn is_only_pigment(recipe: &[(usize, f32)], slot: usize) -> bool {
    let mut present = recipe.iter().filter(|(_, parts)| *parts > 0.0).peekable();
    present.peek().is_some() && present.all(|(chosen, _)| *chosen == slot)
}

fn recipe_after_mixing(recipe: &[(usize, f32)], slot: usize) -> Vec<(usize, f32)> {
    let total: f32 = recipe.iter().map(|(_, parts)| parts.max(0.0)).sum();
    if total <= 0.0 {
        return vec![(slot, 1.0)];
    }
    let added = PARTIAL_DIP * total;
    let mut recipe = recipe.to_vec();
    match recipe.iter_mut().find(|(chosen, _)| *chosen == slot) {
        Some((_, parts)) => *parts += added,
        None => recipe.push((slot, added)),
    }
    recipe
}

pub(super) fn plan_pigment_dip(
    recipe: &[(usize, f32)],
    slot: usize,
    click: PigmentClick,
) -> (Vec<(usize, f32)>, PigmentDip) {
    match click {
        PigmentClick::Mix => (
            recipe_after_mixing(recipe, slot),
            PigmentDip::AddOnTop(slot),
        ),
        PigmentClick::Choose if is_only_pigment(recipe, slot) => {
            (recipe.to_vec(), PigmentDip::DipAgain)
        }
        PigmentClick::Choose | PigmentClick::CleanAndLoad => {
            (vec![(slot, 1.0)], PigmentDip::CleanAndLoad)
        }
    }
}

#[cfg(test)]
mod tests;
