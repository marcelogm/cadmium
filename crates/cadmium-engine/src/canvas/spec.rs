use std::collections::HashSet;

use crate::pigments::{MAX_SLOTS, PigmentDb};

use super::TILE_SIZE;

pub const MAX_PAINTING_SIZE: u32 = 4096;
/// The width of the border of canvas around a painting, on every side: whole tiles.
pub const MARGIN_PX: u32 = 128;
/// The margin's own neutral grey ground, the same in every band.
pub const MARGIN_REFLECTANCE: f32 = 0.42;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Primer {
    WhiteGesso,
    SiennaImprimatura,
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
    #[serde(default)]
    pub margin_px: u32,
}

/// Where the painting sits inside the whole canvas surface, in texels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaintingRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// A painting's own size and ground, before the margin is put around it.
#[derive(Clone, Debug, PartialEq)]
pub struct PaintingRequest {
    pub width: u32,
    pub height: u32,
    pub primer: Primer,
    pub weave: Weave,
    pub pigment_slots: Vec<String>,
}

impl CanvasSpec {
    pub fn for_painting(request: PaintingRequest) -> CanvasSpec {
        CanvasSpec {
            width: request.width + 2 * MARGIN_PX,
            height: request.height + 2 * MARGIN_PX,
            primer: request.primer,
            weave: request.weave,
            pigment_slots: request.pigment_slots,
            margin_px: MARGIN_PX,
        }
    }

    pub fn painting_rect(&self) -> PaintingRect {
        PaintingRect {
            x: self.margin_px,
            y: self.margin_px,
            width: self.width.saturating_sub(2 * self.margin_px),
            height: self.height.saturating_sub(2 * self.margin_px),
        }
    }

    pub fn is_in_margin(&self, x: u32, y: u32) -> bool {
        let rect = self.painting_rect();
        let inside_x = (rect.x..rect.x + rect.width).contains(&x);
        let inside_y = (rect.y..rect.y + rect.height).contains(&y);
        !(inside_x && inside_y)
    }

    pub fn validate(&self, db: &PigmentDb) -> Result<(), String> {
        self.validate_size()?;
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

impl CanvasSpec {
    fn validate_size(&self) -> Result<(), String> {
        if !self.margin_px.is_multiple_of(TILE_SIZE) {
            return Err(format!(
                "margin of {} px is not a whole number of {TILE_SIZE} px tiles",
                self.margin_px
            ));
        }
        let both_margins = self.margin_px.checked_mul(2);
        let inside = |side: u32| both_margins.and_then(|margins| side.checked_sub(margins));
        let (width, height) = (inside(self.width), inside(self.height));
        let allowed = 1..=MAX_PAINTING_SIZE;
        match (width, height) {
            (Some(width), Some(height))
                if allowed.contains(&width) && allowed.contains(&height) =>
            {
                Ok(())
            }
            _ => Err(format!(
                "painting size {}×{} is outside 1..={MAX_PAINTING_SIZE}",
                width.unwrap_or(0),
                height.unwrap_or(0)
            )),
        }
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
