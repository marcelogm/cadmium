use crate::pigments::{MAX_SLOTS, PigmentDb};

mod circle;

pub use circle::{
    CircleAxes, CirclePoint, circle_axes, circle_recipe, default_circle, insertion_index,
    nearest_position,
};

/// The share of each pigment slot in a colour; a stored recipe sums to one.
pub type Recipe = [f32; MAX_SLOTS];

/// The whites, which show nothing on a white primer.
pub const WHITE_IDS: [&str; 2] = ["titanium_white", "zinc_white"];
pub const BLACK_ID: &str = "ivory_black";
pub const MAX_DARKEN: f32 = 0.8;
pub const MAX_MY_COLOURS: usize = 64;

const NORMALISED_TOLERANCE: f32 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ColourRef {
    Paint(usize),
    Mine(usize),
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Palette {
    pub my_colours: Vec<Recipe>,
    pub circle: Vec<ColourRef>,
}

impl Palette {
    pub fn starting(db: &PigmentDb, slots: &[String]) -> Palette {
        Palette {
            my_colours: Vec::new(),
            circle: default_circle(db, slots),
        }
    }

    pub fn recipe_of(&self, colour: ColourRef) -> Option<Recipe> {
        match colour {
            ColourRef::Paint(slot) => (slot < MAX_SLOTS).then(|| paint_recipe(slot)),
            ColourRef::Mine(index) => self.my_colours.get(index).copied(),
        }
    }

    /// Stores the recipe normalised and returns its index; `None` for a recipe `normalised`
    /// refuses or when the palette already holds `MAX_MY_COLOURS`.
    pub fn pin(&mut self, recipe: Recipe) -> Option<usize> {
        if self.my_colours.len() >= MAX_MY_COLOURS {
            return None;
        }
        self.my_colours.push(normalised(&recipe)?);
        Some(self.my_colours.len() - 1)
    }

    pub fn unpin(&mut self, index: usize) {
        if index >= self.my_colours.len() {
            return;
        }
        self.my_colours.remove(index);
        self.circle
            .retain(|colour| *colour != ColourRef::Mine(index));
        for colour in &mut self.circle {
            *colour = after_unpinning(*colour, index);
        }
    }

    /// A colour already on the circle moves to `position` instead of appearing twice.
    pub fn insert_in_circle(&mut self, position: usize, colour: ColourRef) {
        self.circle.retain(|present| *present != colour);
        self.circle.insert(position.min(self.circle.len()), colour);
    }

    pub fn move_in_circle(&mut self, from: usize, to: usize) {
        if from >= self.circle.len() {
            return;
        }
        let colour = self.circle.remove(from);
        self.circle.insert(to.min(self.circle.len()), colour);
    }

    pub fn remove_from_circle(&mut self, position: usize) {
        if position < self.circle.len() {
            self.circle.remove(position);
        }
    }

    /// Checks the palette against a painting of `slot_count` paints; the error says what is wrong.
    pub fn validate(&self, slot_count: usize) -> Result<(), String> {
        self.validate_sizes(slot_count)?;
        (0..self.circle.len())
            .try_for_each(|position| self.validate_circle(position, slot_count))?;
        self.my_colours
            .iter()
            .enumerate()
            .try_for_each(|(index, recipe)| validate_recipe(index, recipe, slot_count))
    }

    /// A circle holds no colour twice, so one longer than the colours there are needs no search.
    fn validate_sizes(&self, slot_count: usize) -> Result<(), String> {
        let pinned = self.my_colours.len();
        if pinned > MAX_MY_COLOURS {
            return Err(format!(
                "palette holds {pinned} pinned colours, the limit is {MAX_MY_COLOURS}"
            ));
        }
        if self.circle.len() > slot_count + pinned {
            return Err(format!(
                "circle holds {} colours, the painting and the palette have {}",
                self.circle.len(),
                slot_count + pinned
            ));
        }
        Ok(())
    }

    fn validate_circle(&self, position: usize, slot_count: usize) -> Result<(), String> {
        let colour = self.circle[position];
        match colour {
            ColourRef::Paint(slot) if slot >= slot_count => Err(format!(
                "circle names paint {slot}, the painting has {slot_count}"
            )),
            ColourRef::Mine(index) if index >= self.my_colours.len() => Err(format!(
                "circle names my colour {index}, the palette has {}",
                self.my_colours.len()
            )),
            _ if self.circle[..position].contains(&colour) => {
                Err(format!("circle holds {} twice", in_words(colour)))
            }
            _ => Ok(()),
        }
    }
}

fn in_words(colour: ColourRef) -> String {
    match colour {
        ColourRef::Paint(slot) => format!("paint {slot}"),
        ColourRef::Mine(index) => format!("my colour {index}"),
    }
}

fn after_unpinning(colour: ColourRef, unpinned: usize) -> ColourRef {
    match colour {
        ColourRef::Mine(later) if later > unpinned => ColourRef::Mine(later - 1),
        kept => kept,
    }
}

fn validate_recipe(index: usize, recipe: &Recipe, slot_count: usize) -> Result<(), String> {
    if !recipe.iter().all(is_share) {
        return Err(format!(
            "my colour {index} has a share that is negative or not a number"
        ));
    }
    if (recipe.iter().sum::<f32>() - 1.0).abs() > NORMALISED_TOLERANCE {
        return Err(format!("my colour {index} does not sum to one"));
    }
    match recipe.iter().rposition(|share| *share != 0.0) {
        Some(slot) if slot >= slot_count => Err(format!(
            "my colour {index} uses paint {slot}, the painting has {slot_count}"
        )),
        _ => Ok(()),
    }
}

/// The recipe scaled to sum to one; `None` when it holds no paint or a negative or non-finite share.
pub fn normalised(recipe: &Recipe) -> Option<Recipe> {
    if !recipe.iter().all(is_share) {
        return None;
    }
    let total: f32 = recipe.iter().sum();
    (total > 0.0 && total.is_finite()).then(|| recipe.map(|share| share / total))
}

fn is_share(share: &f32) -> bool {
    share.is_finite() && *share >= 0.0
}

/// The recipe of one paint alone; the caller keeps `slot < MAX_SLOTS`.
pub fn paint_recipe(slot: usize) -> Recipe {
    std::array::from_fn(|index| if index == slot { 1.0 } else { 0.0 })
}

#[cfg(test)]
mod tests;
