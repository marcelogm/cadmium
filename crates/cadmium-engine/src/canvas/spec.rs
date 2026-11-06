use std::collections::HashSet;

use crate::pigments::{MAX_SLOTS, PigmentDb};

pub const MAX_CANVAS_SIZE: u32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Primer {
    WhiteGesso,
    SiennaImprimatura,
    Tray,
    /// A bright, plain ground for previews painted by hand: gesso's white lifted to paper.
    /// Not offered for paintings.
    Paper,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Weave {
    Linen,
    Cotton,
    Smooth,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanvasSpec {
    pub width: u32,
    pub height: u32,
    pub primer: Primer,
    pub weave: Weave,
    pub pigment_slots: Vec<String>,
}

impl CanvasSpec {
    pub fn validate(&self, db: &PigmentDb) -> Result<(), String> {
        let allowed = 1..=MAX_CANVAS_SIZE;
        if !allowed.contains(&self.width) || !allowed.contains(&self.height) {
            return Err(format!(
                "canvas size {}×{} is outside 1..={MAX_CANVAS_SIZE}",
                self.width, self.height
            ));
        }
        if !(1..=MAX_SLOTS).contains(&self.pigment_slots.len()) {
            return Err(format!(
                "canvas has {} pigment slots, expected 1..={MAX_SLOTS}",
                self.pigment_slots.len()
            ));
        }
        let mut seen = HashSet::new();
        self.pigment_slots
            .iter()
            .try_for_each(|id| validate_slot(db, &mut seen, id))
    }
}

fn validate_slot<'a>(
    db: &PigmentDb,
    seen: &mut HashSet<&'a str>,
    id: &'a str,
) -> Result<(), String> {
    if db.get(id).is_none() {
        return Err(format!("unknown pigment {id:?}"));
    }
    if !seen.insert(id) {
        return Err(format!("pigment {id:?} appears in more than one slot"));
    }
    Ok(())
}
