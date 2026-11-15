//! The wheel as a coarse grid of colours, mixed from the painting's own paints.

use cadmium_engine::palette::{CircleAxes, CirclePoint, Palette, circle_recipe};
use cadmium_engine::pigments::{PigmentDb, mixture_srgb8};

use super::geometry::turn_and_radius_at;

/// The wheel is mixed once per cell of a `WHEEL_CELLS` × `WHEEL_CELLS` grid, not per pixel.
pub const WHEEL_CELLS: usize = 96;

const ARGB32_BYTES: usize = 4;
const OPAQUE: u32 = 0xFF00_0000;

pub struct WheelPaint<'a> {
    pub db: &'a PigmentDb,
    pub slots: &'a [String],
    pub palette: &'a Palette,
    pub axes: CircleAxes,
    pub darken: f32,
}

/// `cells` in row order, `None` outside the disc; `rim` is the circle's colours in order.
#[derive(Clone, Debug, PartialEq)]
pub struct WheelImage {
    pub cells: Vec<Option<[u8; 3]>>,
    pub rim: Vec<[u8; 3]>,
}

impl WheelImage {
    pub fn has_colour(&self) -> bool {
        self.cells.iter().any(Option::is_some)
    }

    /// The grid as Cairo's ARGB32 rows of `stride` bytes; a cell with no colour is transparent.
    pub fn argb32(&self, stride: usize) -> Vec<u8> {
        let mut bytes = vec![0; stride * WHEEL_CELLS];
        let rows = bytes.chunks_exact_mut(stride);
        for (row, cells) in rows.zip(self.cells.as_chunks::<WHEEL_CELLS>().0) {
            write_argb32_row(row, cells);
        }
        bytes
    }
}

fn write_argb32_row(row: &mut [u8], cells: &[Option<[u8; 3]>; WHEEL_CELLS]) {
    for (pixel, cell) in row.as_chunks_mut::<ARGB32_BYTES>().0.iter_mut().zip(cells) {
        *pixel = argb32_pixel(*cell).to_ne_bytes();
    }
}

/// An empty circle, or one naming a colour that is gone, draws no cell and no rim.
pub fn wheel_image(paint: &WheelPaint<'_>) -> WheelImage {
    let cells = (0..WHEEL_CELLS * WHEEL_CELLS)
        .map(|index| cell_colour(paint, index % WHEEL_CELLS, index / WHEEL_CELLS))
        .collect();
    let recipes = paint.palette.circle.iter();
    let rim: Option<Vec<[u8; 3]>> = recipes
        .map(|colour| {
            let recipe = paint.palette.recipe_of(*colour)?;
            Some(mixture_srgb8(paint.db, paint.slots, &recipe))
        })
        .collect();
    WheelImage {
        cells,
        rim: rim.unwrap_or_default(),
    }
}

fn cell_colour(paint: &WheelPaint<'_>, column: usize, row: usize) -> Option<[u8; 3]> {
    let (x, y) = (column as f64 + 0.5, row as f64 + 0.5);
    let (turn, radius) = turn_and_radius_at(x, y, WHEEL_CELLS as f64)?;
    let point = CirclePoint {
        turn,
        radius,
        darken: paint.darken,
    };
    let recipe = circle_recipe(paint.palette, paint.axes, point)?;
    Some(mixture_srgb8(paint.db, paint.slots, &recipe))
}

fn argb32_pixel(cell: Option<[u8; 3]>) -> u32 {
    cell.map_or(0, |[red, green, blue]| {
        OPAQUE | u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue)
    })
}

#[cfg(test)]
mod tests;
