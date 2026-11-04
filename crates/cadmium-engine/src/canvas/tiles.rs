use super::TILE_SIZE;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileCoord {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Debug)]
pub struct DirtyTiles {
    width: u32,
    height: u32,
    columns: u32,
    rows: u32,
    marked: Vec<bool>,
}

impl DirtyTiles {
    pub fn new(width: u32, height: u32) -> Self {
        let columns = width.div_ceil(TILE_SIZE);
        let rows = height.div_ceil(TILE_SIZE);
        DirtyTiles {
            width,
            height,
            columns,
            rows,
            marked: vec![false; (columns * rows) as usize],
        }
    }

    pub fn mark_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        for tile in tiles_in_rect(self.width, self.height, x0, y0, x1, y1) {
            let index = self.index(tile);
            self.marked[index] = true;
        }
    }

    pub fn mark_all(&mut self) {
        self.marked.fill(true);
    }

    pub fn take(&mut self) -> Vec<TileCoord> {
        let tiles = self.tiles();
        self.clear();
        tiles
    }

    #[expect(clippy::excessive_nesting)]
    pub fn tiles(&self) -> Vec<TileCoord> {
        let mut tiles = Vec::new();
        for x in 0..self.columns {
            for y in 0..self.rows {
                let tile = TileCoord { x, y };
                if self.marked[self.index(tile)] {
                    tiles.push(tile);
                }
            }
        }
        tiles
    }

    pub fn clear(&mut self) {
        self.marked.fill(false);
    }

    pub fn is_empty(&self) -> bool {
        !self.marked.contains(&true)
    }

    pub fn contains(&self, tile: TileCoord) -> bool {
        tile.x < self.columns && tile.y < self.rows && self.marked[self.index(tile)]
    }

    fn index(&self, tile: TileCoord) -> usize {
        (tile.y * self.columns + tile.x) as usize
    }
}

#[expect(clippy::too_many_arguments)]
pub fn tiles_in_rect(
    width: u32,
    height: u32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
) -> Vec<TileCoord> {
    let (Some(columns), Some(rows)) = (tile_span(width, x0, x1), tile_span(height, y0, y1)) else {
        return Vec::new();
    };
    columns
        .flat_map(|x| rows.clone().map(move |y| TileCoord { x, y }))
        .collect()
}

fn tile_span(length: u32, a: f32, b: f32) -> Option<std::ops::RangeInclusive<u32>> {
    if length == 0 || a.is_nan() || b.is_nan() {
        return None;
    }
    let last_pixel = (length - 1) as f32;
    let first_pixel = a.min(b).floor();
    let final_pixel = a.max(b).floor();
    if final_pixel < 0.0 || first_pixel > last_pixel {
        return None;
    }
    let first = first_pixel.max(0.0) as u32 / TILE_SIZE;
    let last = final_pixel.min(last_pixel) as u32 / TILE_SIZE;
    Some(first..=last)
}
