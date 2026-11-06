//! Gridded paper: under the example strokes until they are first painted, and where the
//! test sheet will be painted (blank for now).

use adw::prelude::*;
use gtk::cairo;

/// The test sheet's paper and its two grids (the approved mockup's `.test`).
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

/// The blank paper where the test sheet will be: it takes the room left, `margin` in.
pub(super) fn test_sheet_placeholder(margin: i32) -> gtk::DrawingArea {
    let area = paper_area();
    area.set_hexpand(true);
    area.set_vexpand(true);
    area.set_margin_top(margin);
    area.set_margin_bottom(margin);
    area.set_margin_start(margin);
    area.set_margin_end(margin);
    area
}

/// A drawing area shaped like a libadwaita card: its corners rounded, the paper clipped to them.
fn paper_area() -> gtk::DrawingArea {
    let area = grid_area();
    area.add_css_class("card");
    area.set_overflow(gtk::Overflow::Hidden);
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
pub(super) fn draw_grid(context: &cairo::Context, size: [f64; 2]) {
    set_colour(context, PAPER);
    let _ = context.paint();
    context.set_line_width(LINE_WIDTH);
    for (step, colour) in [(MINOR_STEP, MINOR_LINE), (MAJOR_STEP, MAJOR_LINE)] {
        grid_lines(context, size, step);
        set_colour(context, colour);
        let _ = context.stroke();
    }
}

/// One line every `step` across and down, on pixel centres so each is one pixel wide.
fn grid_lines(context: &cairo::Context, [width, height]: [f64; 2], step: f64) {
    let mut x = step;
    while x < width {
        context.move_to(x + 0.5, 0.0);
        context.line_to(x + 0.5, height);
        x += step;
    }
    let mut y = step;
    while y < height {
        context.move_to(0.0, y + 0.5);
        context.line_to(width, y + 0.5);
        y += step;
    }
}

fn set_colour(context: &cairo::Context, [red, green, blue]: [f64; 3]) {
    context.set_source_rgb(red, green, blue);
}
