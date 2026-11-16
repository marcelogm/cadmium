use std::collections::HashSet;

use crate::pigments::{MAX_SLOTS, PigmentDb};

use super::{TILE_SIZE, TexelBlock, TileCoord, tiles_in_rect};

pub const MAX_PAINTING_SIZE: u32 = 4096;
/// The width of the border of canvas around a painting, on every side: whole tiles.
pub const MARGIN_PX: u32 = 128;
/// The margin's own neutral grey ground, the same in every band.
pub const MARGIN_REFLECTANCE: f32 = 0.42;

const _: () = assert!(MARGIN_PX.is_multiple_of(TILE_SIZE));

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

impl PaintingRect {
    /// Whether every texel of `[x0, y0, x1, y1]`, corners included, lies on the painting.
    pub fn contains_rect(&self, [x0, y0, x1, y1]: [f32; 4]) -> bool {
        let (left, top) = (self.x as f32, self.y as f32);
        let right = (self.x + self.width) as f32;
        let bottom = (self.y + self.height) as f32;
        x0.min(x1) >= left && y0.min(y1) >= top && x0.max(x1) < right && y0.max(y1) < bottom
    }
}

impl CanvasSpec {
    pub fn for_painting(request: PaintingRequest) -> CanvasSpec {
        CanvasSpec {
            width: request.width.saturating_add(2 * MARGIN_PX),
            height: request.height.saturating_add(2 * MARGIN_PX),
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
            width: self.inner_side(self.width).unwrap_or(0),
            height: self.inner_side(self.height).unwrap_or(0),
        }
    }

    pub fn is_in_margin(&self, x: u32, y: u32) -> bool {
        let rect = self.painting_rect();
        let inside_x = (rect.x..rect.x + rect.width).contains(&x);
        let inside_y = (rect.y..rect.y + rect.height).contains(&y);
        !(inside_x && inside_y)
    }

    /// The four bands of the margin, top and bottom across the whole width, left and right
    /// between them; none without a margin.
    pub(crate) fn margin_strips(&self) -> Vec<TexelBlock> {
        let margin = self.margin_px;
        if margin == 0 {
            return Vec::new();
        }
        let Some(inner_height) = self.inner_side(self.height) else {
            return Vec::new();
        };
        let across = [self.width, margin];
        let down = [margin, inner_height];
        [
            [0, 0],
            [0, self.height - margin],
            [0, margin],
            [self.width - margin, margin],
        ]
        .into_iter()
        .zip([across, across, down, down])
        .map(|(origin, size)| TexelBlock { origin, size })
        .collect()
    }

    /// The tiles holding at least one texel of the margin.
    pub(crate) fn margin_tiles(&self) -> Vec<TileCoord> {
        let mut tiles: Vec<TileCoord> = self
            .margin_strips()
            .into_iter()
            .flat_map(|strip| tiles_in_rect(self.width, self.height, block_rect(strip)))
            .collect();
        tiles.sort_unstable();
        tiles.dedup();
        tiles
    }

    pub(crate) fn is_tile_wholly_in_margin(&self, tile: TileCoord) -> bool {
        let rect = self.painting_rect();
        let overlaps = |start: u32, painting_start: u32, painting_length: u32| {
            start < painting_start + painting_length && start + TILE_SIZE > painting_start
        };
        !(overlaps(tile.x * TILE_SIZE, rect.x, rect.width)
            && overlaps(tile.y * TILE_SIZE, rect.y, rect.height))
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

    fn inner_side(&self, side: u32) -> Option<u32> {
        self.margin_px
            .checked_mul(2)
            .and_then(|margins| side.checked_sub(margins))
    }

    fn validate_size(&self) -> Result<(), String> {
        if ![0, MARGIN_PX].contains(&self.margin_px) {
            return Err(format!(
                "margin of {} px is neither none nor the {MARGIN_PX} px margin",
                self.margin_px
            ));
        }
        let (Some(width), Some(height)) =
            (self.inner_side(self.width), self.inner_side(self.height))
        else {
            return Err(format!(
                "a margin of {} px leaves no painting inside a {}×{} canvas",
                self.margin_px, self.width, self.height
            ));
        };
        let allowed = 1..=MAX_PAINTING_SIZE;
        if !allowed.contains(&width) || !allowed.contains(&height) {
            return Err(format!(
                "painting size {width}×{height} is outside 1..={MAX_PAINTING_SIZE}"
            ));
        }
        Ok(())
    }
}

fn block_rect(block: TexelBlock) -> [f32; 4] {
    let [x, y] = block.origin;
    let [width, height] = block.size;
    [
        x as f32,
        y as f32,
        (x + width - 1) as f32,
        (y + height - 1) as f32,
    ]
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
