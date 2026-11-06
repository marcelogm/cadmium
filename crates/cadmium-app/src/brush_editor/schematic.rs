//! The tip's technical drawing (D1, "ink"): front and side views with their ferrules, blue
//! dimensions, the footprint and a title block, on light gridded paper — the owner-approved
//! mockup, drawn from `TipGeometry` in sheet units, then fitted into the widget.

use std::rc::Rc;

use adw::prelude::*;
use cadmium_engine::brush::BrushSpec;
use cadmium_studio::TipGeometry;
use gtk::cairo;

use crate::ink::contain;
use crate::surface_scale::SurfaceScaleWatch;

mod canvas;
mod dimensions;
mod footprint;
mod pen;
mod sheet;
mod title_block;
mod views;

use canvas::Canvas;
use pen::{CAPTION, DIMENSION_LABEL, GRID, INK, PAPER, Pen};
use sheet::{Figures, SHEET, Sheet};
use views::Placed;

/// The left column's width, and the sheet's height at it.
const DISPLAY_SIZE: [i32; 2] = [312, 567];
const FRAME_INSET: f64 = 6.0;
const FRAME_PEN: Pen = Pen::new(INK, 0.8);
const GRID_STEP: f64 = 10.0;
const GRID_PEN: Pen = Pen::new(GRID, 0.4);

/// What the schematic draws: the brush, and what its head and hair are called (`None`:
/// Custom, D5 — even when the values happen to equal a preset's).
pub(super) struct Subject<'a> {
    pub(super) brush: &'a BrushSpec,
    pub(super) head: Option<&'static str>,
    pub(super) hair: Option<&'static str>,
}

/// The widget the schematic is drawn in; the editor sets its draw function.
pub(super) fn schematic_area() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(DISPLAY_SIZE[0])
        .content_height(DISPLAY_SIZE[1])
        .accessible_role(gtk::AccessibleRole::Img)
        .build();
    area.update_property(&[gtk::accessible::Property::Label("Drawing of the brush tip")]);
    follow_scale(&area);
    area
}

/// Redraws `area` when it moves to a display of another scale: its lines are snapped to
/// the display's pixels.
fn follow_scale(area: &gtk::DrawingArea) {
    let watch = Rc::new(SurfaceScaleWatch::default());
    let started = Rc::clone(&watch);
    area.connect_realize(move |area| started.start(area, redraw));
    area.connect_unrealize(move |_| watch.stop());
    area.connect_scale_factor_notify(redraw);
}

fn redraw(area: &gtk::DrawingArea) {
    area.queue_draw();
}

/// Draws `subject` fitted into a widget of `size`, on a display of `pixel_scale` device
/// pixels per widget unit.
pub(super) fn draw(context: &cairo::Context, subject: &Subject, size: [i32; 2], pixel_scale: f64) {
    paint(&Canvas::new(context, pixel_scale), subject, size);
}

fn paint(canvas: &Canvas, subject: &Subject, size: [i32; 2]) {
    let fit = contain([0.0; 2], SHEET, size.map(f64::from), 0.0);
    canvas.translate(fit.origin[0], fit.origin[1]);
    canvas.scale(fit.scale, fit.scale);
    paper(canvas);
    let tip = TipGeometry::new(&subject.brush.head, &subject.brush.hair);
    let sheet = Sheet::of(&tip, &figures(canvas, &tip));
    views::draw_view(canvas, &Placed::front(&tip, &sheet));
    views::draw_view(canvas, &Placed::side(&tip, &sheet));
    dimensions::draw(canvas, &tip, &sheet);
    footprint::draw(canvas, &tip, &sheet);
    title_block::draw(canvas, &title_block::cells(subject));
}

/// The figures' widths, as the pen sets them on `context`.
fn figures(context: &cairo::Context, tip: &TipGeometry) -> Figures {
    let tip_width = dimensions::tip_width_label(tip);
    Figures {
        footprint_caption: pen::width(context, footprint::CAPTION_TEXT, &CAPTION),
        tip_width: pen::width(context, &tip_width, &DIMENSION_LABEL),
    }
}

/// The paper, its grid, and the frame inside its edge.
fn paper(canvas: &Canvas) {
    let [width, height] = SHEET;
    let [red, green, blue] = PAPER;
    canvas.set_source_rgb(red, green, blue);
    canvas.rectangle(0.0, 0.0, width, height);
    let _ = canvas.fill();
    grid_lines(canvas);
    let inset = [FRAME_INSET; 2];
    let far = [width - FRAME_INSET, height - FRAME_INSET];
    pen::rectangle(canvas, inset, far, FRAME_PEN);
}

/// One line every `GRID_STEP` across and down, each on whole device pixels.
fn grid_lines(canvas: &Canvas) {
    let [width, height] = SHEET;
    pen::ink(canvas, GRID_PEN);
    let mut x = GRID_STEP;
    while x < width {
        segment(canvas, [x, 0.0], [x, height]);
        x += GRID_STEP;
    }
    let mut y = GRID_STEP;
    while y < height {
        segment(canvas, [0.0, y], [width, y]);
        y += GRID_STEP;
    }
    let _ = canvas.stroke();
}

fn segment(canvas: &Canvas, from: [f64; 2], to: [f64; 2]) {
    let [from, to] = [from, to].map(|end| canvas.snapped(end, GRID_PEN.width()));
    canvas.move_to(from[0], from[1]);
    canvas.line_to(to[0], to[1]);
}

#[cfg(test)]
mod tests;
