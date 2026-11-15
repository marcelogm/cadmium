//! The wheel as a coarse grid of colours, mixed from the painting's own paints.

use cadmium_engine::palette::{CircleAxes, CirclePoint, Palette, circle_recipe};
use cadmium_engine::pigments::{PigmentDb, mixture_srgb8};

use super::geometry::turn_and_radius_at;

/// The wheel is mixed once per cell of a `WHEEL_CELLS` × `WHEEL_CELLS` grid, not per pixel.
pub(super) const WHEEL_CELLS: usize = 96;

const ARGB32_BYTES: usize = 4;
pub(super) const ARGB32_STRIDE: usize = WHEEL_CELLS * ARGB32_BYTES;
/// How far past the disc its colours are painted: scaled up, a pixel on the rim blends the cells
/// around it, and one of those with no colour would let the disc underneath show through.
const RIM_BLEED_CELLS: usize = 2;
const NEIGHBOURS: [(isize, isize); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

type Cell = Option<[u8; 3]>;
const OPAQUE: u32 = 0xFF00_0000;

pub(crate) struct WheelPaint<'a> {
    pub(crate) db: &'a PigmentDb,
    pub(crate) slots: &'a [String],
    pub(crate) palette: &'a Palette,
    pub(crate) axes: CircleAxes,
    pub(crate) darken: f32,
}

/// `cells` in row order, `None` outside the disc; `rim` is the circle's colours in order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WheelImage {
    pub(crate) cells: Vec<Cell>,
    pub(crate) rim: Vec<[u8; 3]>,
}

impl WheelImage {
    pub(crate) fn has_colour(&self) -> bool {
        self.cells.iter().any(Option::is_some)
    }

    /// The grid as Cairo's ARGB32 rows of `ARGB32_STRIDE` bytes. A cell with no colour takes the
    /// mean of its coloured neighbours, `RIM_BLEED_CELLS` deep; farther from any it is transparent.
    pub(crate) fn argb32(&self) -> Vec<u8> {
        let painted = (0..RIM_BLEED_CELLS).fold(self.cells.clone(), |cells, _| bled(&cells));
        let mut bytes = vec![0; ARGB32_STRIDE * WHEEL_CELLS];
        let rows = bytes.as_chunks_mut::<ARGB32_STRIDE>().0;
        for (row, cells) in rows.iter_mut().zip(painted.as_chunks::<WHEEL_CELLS>().0) {
            write_argb32_row(row, cells);
        }
        bytes
    }
}

fn bled(cells: &[Cell]) -> Vec<Cell> {
    (0..cells.len())
        .map(|index| cells[index].or_else(|| mean_of_neighbours(cells, index)))
        .collect()
}

fn mean_of_neighbours(cells: &[Cell], index: usize) -> Cell {
    let coloured: Vec<[u8; 3]> = NEIGHBOURS
        .iter()
        .filter_map(|offset| neighbour(cells, index, *offset))
        .collect();
    let count = u32::try_from(coloured.len())
        .ok()
        .filter(|count| *count > 0)?;
    Some(std::array::from_fn(|channel| {
        let total: u32 = coloured
            .iter()
            .map(|colour| u32::from(colour[channel]))
            .sum();
        u8::try_from((total + count / 2) / count).unwrap_or(u8::MAX)
    }))
}

fn neighbour(cells: &[Cell], index: usize, (right, down): (isize, isize)) -> Cell {
    let column = (index % WHEEL_CELLS).checked_add_signed(right)?;
    let row = (index / WHEEL_CELLS).checked_add_signed(down)?;
    let cell = cells.get(row * WHEEL_CELLS + column)?;
    cell.filter(|_| column < WHEEL_CELLS)
}

fn write_argb32_row(row: &mut [u8], cells: &[Cell; WHEEL_CELLS]) {
    for (pixel, cell) in row.as_chunks_mut::<ARGB32_BYTES>().0.iter_mut().zip(cells) {
        *pixel = argb32_pixel(*cell).to_ne_bytes();
    }
}

/// An empty circle, or one naming a colour that is gone, draws no cell and no rim.
pub(crate) fn wheel_image(paint: &WheelPaint<'_>) -> WheelImage {
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

fn cell_colour(paint: &WheelPaint<'_>, column: usize, row: usize) -> Cell {
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

fn argb32_pixel(cell: Cell) -> u32 {
    cell.map_or(0, |[red, green, blue]| {
        OPAQUE | u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue)
    })
}

#[cfg(test)]
mod tests;
