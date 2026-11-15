use super::{BLACK_ID, ColourRef, MAX_DARKEN, Palette, Recipe, WHITE_IDS, paint_recipe};
use crate::pigments::PigmentDb;

const HUE_SEXTANT_DEG: f32 = 60.0;

/// A point of the colour circle: `turn` is 0 at the top and grows clockwise, in turns;
/// `radius` is 0 at the centre and 1 on the rim; `darken` runs from 0 to `MAX_DARKEN`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CirclePoint {
    pub turn: f32,
    pub radius: f32,
    pub darken: f32,
}

/// The slots of the white the centre holds and of the black darkening adds, when the painting has them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CircleAxes {
    pub white: Option<usize>,
    pub black: Option<usize>,
}

pub fn circle_axes(slots: &[String]) -> CircleAxes {
    CircleAxes {
        white: slots.iter().position(|id| WHITE_IDS.contains(&id.as_str())),
        black: slots.iter().position(|id| id == BLACK_ID),
    }
}

/// The coloured paints of the painting, by rising hue of their masstone.
pub fn default_circle(db: &PigmentDb, slots: &[String]) -> Vec<ColourRef> {
    let mut coloured: Vec<(f32, usize)> = slots
        .iter()
        .enumerate()
        .filter(|(_, id)| !WHITE_IDS.contains(&id.as_str()) && *id != BLACK_ID)
        .map(|(slot, id)| (hue_deg(db.masstone_srgb8(id)), slot))
        .collect();
    coloured.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    coloured
        .into_iter()
        .map(|(_, slot)| ColourRef::Paint(slot))
        .collect()
}

/// The mixture at a point of the circle; `None` for an empty circle or one naming a colour that is gone.
pub fn circle_recipe(palette: &Palette, axes: CircleAxes, point: CirclePoint) -> Option<Recipe> {
    let len = palette.circle.len();
    if len == 0 {
        return None;
    }
    let recipes: Vec<Recipe> = palette
        .circle
        .iter()
        .map(|colour| palette.recipe_of(*colour))
        .collect::<Option<_>>()?;
    let position = point.turn.rem_euclid(1.0) * len as f32;
    let before = position.floor() as usize % len;
    let after = (before + 1) % len;
    let towards_after = position.fract();
    let radius = axes.white.map_or(1.0, |_| point.radius.clamp(0.0, 1.0));
    let darken = axes
        .black
        .map_or(0.0, |_| point.darken.clamp(0.0, MAX_DARKEN));
    let white = axes.white.map_or([0.0; _], paint_recipe);
    let black = axes.black.map_or([0.0; _], paint_recipe);
    Some(std::array::from_fn(|slot| {
        let rim =
            recipes[before][slot] * (1.0 - towards_after) + recipes[after][slot] * towards_after;
        rim * radius * (1.0 - darken)
            + white[slot] * (1.0 - radius) * (1.0 - darken)
            + black[slot] * darken
    }))
}

/// Where a colour dropped at `turn` enters a circle of `len` colours.
pub fn insertion_index(turn: f32, len: usize) -> usize {
    nearest_rim_step(turn, len) % (len + 1)
}

/// The circle colour nearest `turn`; `None` for an empty circle.
pub fn nearest_position(turn: f32, len: usize) -> Option<usize> {
    (len > 0).then(|| nearest_rim_step(turn, len) % len)
}

fn nearest_rim_step(turn: f32, len: usize) -> usize {
    (turn.rem_euclid(1.0) * len as f32).round() as usize
}

fn hue_deg(srgb8: [u8; 3]) -> f32 {
    let [red, green, blue] = srgb8.map(f32::from);
    let strongest = red.max(green).max(blue);
    let chroma = strongest - red.min(green).min(blue);
    if chroma == 0.0 {
        return 0.0;
    }
    let sextant = if strongest == red {
        ((green - blue) / chroma).rem_euclid(6.0)
    } else if strongest == green {
        (blue - red) / chroma + 2.0
    } else {
        (red - green) / chroma + 4.0
    };
    sextant * HUE_SEXTANT_DEG
}

#[cfg(test)]
mod tests;
