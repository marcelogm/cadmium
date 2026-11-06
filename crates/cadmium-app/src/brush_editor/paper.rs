//! Gridded paper: under the example strokes until they are first painted, and on the test
//! sheet until its first frame, there with its lines where the sheet's ground rules them.

use adw::prelude::*;
use cadmium_engine::present::ViewTransform;
use cadmium_studio::{GROUND_COARSE_STEP_PX, GROUND_FINE_STEP_PX, GROUND_LINE_PX};
use gtk::cairo;

/// The editor's paper colour, its two grid colours, and the example strokes' paper's grid:
/// minor lines every 16 px and major every 80.
const PAPER: [f64; 3] = rgb(0xfbfaf6);
const MINOR_LINE: [f64; 3] = rgb(0xe3e6ea);
const MAJOR_LINE: [f64; 3] = rgb(0xcfd5dc);
const MINOR_STEP: f64 = 16.0;
const MAJOR_STEP: f64 = 80.0;
const LINE_WIDTH: f64 = 1.0;
/// The example strokes' picture under the schematic: the 910 × 440 sheet at the column's width.
const EXAMPLES_SIZE: [i32; 2] = [312, 151];

pub(super) const fn rgb(hex: u32) -> [f64; 3] {
    [
        ((hex >> 16) & 0xff) as f64 / 255.0,
        ((hex >> 8) & 0xff) as f64 / 255.0,
        (hex & 0xff) as f64 / 255.0,
    ]
}

/// The blank paper under the example strokes; it gives their picture its size. The card
/// holding the picture rounds and clips it.
pub(super) fn examples_paper() -> gtk::DrawingArea {
    let area = grid_area();
    area.set_content_width(EXAMPLES_SIZE[0]);
    area.set_content_height(EXAMPLES_SIZE[1]);
    area
}

/// A drawing area painted with gridded paper.
fn grid_area() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    area.set_draw_func(|_, context, width, height| {
        draw_grid(context, [f64::from(width), f64::from(height)]);
    });
    area
}

/// Paper filling `size`, a fine grid, and a coarser one over it.
fn draw_grid(context: &cairo::Context, size: [f64; 2]) {
    let lines = [MINOR_STEP, MAJOR_STEP].map(|step| paper_lines(size, step));
    draw_paper(context, size, &lines);
}

/// The test sheet's paper filling `size` widget px, before its first frame: the grid where
/// the sheet's ground rules it, shown through `view` at `scale` device px per widget px.
pub(super) fn draw_sheet_grid(
    context: &cairo::Context,
    size: [f64; 2],
    view: ViewTransform,
    scale: f64,
) {
    let lines = [GROUND_FINE_STEP_PX, GROUND_COARSE_STEP_PX]
        .map(|step| ground_lines(size, view, scale, step));
    draw_paper(context, size, &lines);
}

/// Paper filling `size`, the fine lines, and the coarse ones over them; each set's lines
/// are given across, then down.
fn draw_paper(context: &cairo::Context, size: [f64; 2], [fine, coarse]: &[[Vec<f64>; 2]; 2]) {
    set_colour(context, PAPER);
    let _ = context.paint();
    context.set_line_width(LINE_WIDTH);
    for (lines, colour) in [(fine, MINOR_LINE), (coarse, MAJOR_LINE)] {
        trace_lines(context, size, lines);
        set_colour(context, colour);
        let _ = context.stroke();
    }
}

/// One line every `step` across and down `size`, on pixel centres so each is one pixel wide.
fn paper_lines(size: [f64; 2], step: f64) -> [Vec<f64>; 2] {
    size.map(|extent| {
        std::iter::successors(Some(step), |at| Some(at + step))
            .take_while(|&at| at < extent)
            .map(|at| at + 0.5)
            .collect()
    })
}

/// The test sheet ground's lines every `step` canvas px that fall in a widget of `size`
/// showing the canvas through `view` at `scale` device px per widget px: their centres, in
/// widget px, across then down.
fn ground_lines(size: [f64; 2], view: ViewTransform, scale: f64, step: u32) -> [Vec<f64>; 2] {
    let per_canvas_px = f64::from(view.zoom) / scale;
    let origins = [f64::from(view.origin_x), f64::from(view.origin_y)];
    std::array::from_fn(|axis| axis_lines(origins[axis], per_canvas_px, size[axis], step))
}

/// One axis of `ground_lines`: the canvas from `origin` on, `per_canvas_px` widget px to a
/// canvas px, across `extent` widget px.
fn axis_lines(origin: f64, per_canvas_px: f64, extent: f64, step: u32) -> Vec<f64> {
    let centre = f64::from(GROUND_LINE_PX) / 2.0;
    let step = f64::from(step);
    let first = ((origin - centre) / step).ceil().max(0.0);
    std::iter::successors(Some(first * step + centre), |at| Some(at + step))
        .map(|at| (at - origin) * per_canvas_px)
        .take_while(|&at| at < extent)
        .collect()
}

/// The upright lines at `across` and the level ones at `down`, spanning `size`.
fn trace_lines(
    context: &cairo::Context,
    [width, height]: [f64; 2],
    [across, down]: &[Vec<f64>; 2],
) {
    for &x in across {
        context.move_to(x, 0.0);
        context.line_to(x, height);
    }
    for &y in down {
        context.move_to(0.0, y);
        context.line_to(width, y);
    }
}

fn set_colour(context: &cairo::Context, [red, green, blue]: [f64; 3]) {
    context.set_source_rgb(red, green, blue);
}

#[cfg(test)]
mod tests;
