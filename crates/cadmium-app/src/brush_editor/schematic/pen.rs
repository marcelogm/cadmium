//! The schematic's pen (D1, "ink"): colours, lines, arrows, dimension lines and lettering.
//! Straight lines across or down are snapped to device pixels, so thin ones stay crisp.

use std::f64::consts::{FRAC_PI_2, PI};

use gtk::cairo;

use super::super::paper::rgb;
use super::canvas::{Bounds, Canvas, Part, union};

pub(super) const PAPER: [f64; 3] = rgb(0xf5f3ec);
pub(super) const INK: [f64; 3] = rgb(0x1f2a36);
pub(super) const THIN: [f64; 3] = rgb(0x6d7782);
pub(super) const DIMENSION: [f64; 3] = rgb(0x2f6fd6);
pub(super) const GRID: [f64; 3] = rgb(0xe7e3d8);
/// The dash-dot of a centre line.
pub(super) const CENTRE_DASH: [f64; 4] = [7.0, 2.0, 1.5, 2.0];
const DIMENSION_PEN: Pen = Pen::new(DIMENSION, 0.8);
const EXTENSION_PEN: Pen = Pen::new(DIMENSION, 0.5);
const ARROW_PEN: Pen = Pen::new(DIMENSION, 0.9);
const ARROW_SIZE: f64 = 4.5;
/// How far each wing of an arrowhead turns back from where it points, in radians.
const ARROW_SPREAD: f64 = 2.7;
const EXTENSION_OVERSHOOT: f64 = 3.0;
const LABEL_BELOW: f64 = 11.0;
const LABEL_BESIDE: f64 = 4.0;
const LABEL_LIFT: f64 = 2.0;
const FONT: &str = "monospace";

/// A colour and a line width.
#[derive(Clone, Copy)]
pub(super) struct Pen {
    colour: [f64; 3],
    width: f64,
}

impl Pen {
    pub(super) const fn new(colour: [f64; 3], width: f64) -> Pen {
        Pen { colour, width }
    }

    pub(super) const fn width(self) -> f64 {
        self.width
    }
}

#[derive(Clone, Copy)]
pub(super) enum Anchor {
    Start,
    Middle,
}

/// How a line of capitals is set: size, extra space between letters (in ems), colour, anchor.
pub(super) struct Lettering {
    pub(super) size: f64,
    pub(super) spacing_em: f64,
    pub(super) colour: [f64; 3],
    pub(super) anchor: Anchor,
}

/// A view's or the footprint's caption.
pub(super) const CAPTION: Lettering = Lettering {
    size: 8.0,
    spacing_em: 0.2,
    colour: THIN,
    anchor: Anchor::Middle,
};

/// A dimension's figure.
pub(super) const DIMENSION_LABEL: Lettering = Lettering {
    size: 8.0,
    spacing_em: 0.04,
    colour: DIMENSION,
    anchor: Anchor::Middle,
};

/// A dimension: where its line runs (from, to, both across or both down), its figure, and
/// where its two extension lines start (a y each for a line across, an x each for one down).
pub(super) struct Dimension<'a> {
    pub(super) from: [f64; 2],
    pub(super) to: [f64; 2],
    pub(super) label: &'a str,
    pub(super) extension_from: [f64; 2],
}

pub(super) fn ink(canvas: &Canvas, pen: Pen) {
    let [red, green, blue] = pen.colour;
    canvas.set_source_rgb(red, green, blue);
    canvas.set_line_width(pen.width);
}

/// A straight line; one across or down lands on whole device pixels.
pub(super) fn line(canvas: &Canvas, from: [f64; 2], to: [f64; 2], pen: Pen) {
    let straight = from[0] == to[0] || from[1] == to[1];
    let [from, to] = if straight {
        [from, to].map(|end| canvas.snapped(end, pen.width))
    } else {
        [from, to]
    };
    ink(canvas, pen);
    canvas.move_to(from[0], from[1]);
    canvas.line_to(to[0], to[1]);
    let _ = canvas.stroke();
}

pub(super) fn dashed(canvas: &Canvas, from: [f64; 2], to: [f64; 2], dash: &[f64], pen: Pen) {
    canvas.set_dash(dash, 0.0);
    line(canvas, from, to, pen);
    canvas.set_dash(&[], 0.0);
}

/// A rectangle from `min` to `max`, its edges on whole device pixels.
pub(super) fn rectangle(canvas: &Canvas, min: [f64; 2], max: [f64; 2], pen: Pen) {
    let [min, max] = [min, max].map(|corner| canvas.snapped(corner, pen.width));
    ink(canvas, pen);
    canvas.rectangle(min[0], min[1], max[0] - min[0], max[1] - min[1]);
    let _ = canvas.stroke();
}

/// Fills the current path with `fill` at `alpha`, then strokes it with `stroke`.
pub(super) fn fill_and_stroke(canvas: &Canvas, fill: [f64; 3], alpha: f64, stroke: Pen) {
    let [red, green, blue] = fill;
    canvas.set_source_rgba(red, green, blue, alpha);
    let _ = canvas.fill_preserve();
    ink(canvas, stroke);
    let _ = canvas.stroke();
}

/// A quadratic Bézier from the current point `from`, pulled towards `control`, to `to`.
pub(super) fn quad_to(context: &cairo::Context, from: [f64; 2], control: [f64; 2], to: [f64; 2]) {
    let towards = |a: [f64; 2]| {
        [
            a[0] + 2.0 / 3.0 * (control[0] - a[0]),
            a[1] + 2.0 / 3.0 * (control[1] - a[1]),
        ]
    };
    let [first, second] = [towards(from), towards(to)];
    context.curve_to(first[0], first[1], second[0], second[1], to[0], to[1]);
}

/// An open arrowhead at `tip`, pointing along `angle`.
fn arrow(canvas: &Canvas, tip: [f64; 2], angle: f64) {
    let wing = |turn: f64| {
        [
            tip[0] + (angle + turn).cos() * ARROW_SIZE,
            tip[1] + (angle + turn).sin() * ARROW_SIZE,
        ]
    };
    let [left, right] = [wing(ARROW_SPREAD), wing(-ARROW_SPREAD)];
    ink(canvas, ARROW_PEN);
    canvas.move_to(left[0], left[1]);
    canvas.line_to(tip[0], tip[1]);
    canvas.line_to(right[0], right[1]);
    let _ = canvas.stroke();
}

/// The box a dimension line from `from` to `to` and its arrowheads cover, in the widget.
fn line_and_arrows(canvas: &Canvas, from: [f64; 2], to: [f64; 2]) -> Bounds {
    let wing = ARROW_SIZE * (PI - ARROW_SPREAD).sin();
    let across = from[1] == to[1];
    let [reach_x, reach_y] = if across { [0.0, wing] } else { [wing, 0.0] };
    let min = [from[0].min(to[0]) - reach_x, from[1].min(to[1]) - reach_y];
    let max = [from[0].max(to[0]) + reach_x, from[1].max(to[1]) + reach_y];
    canvas.on_widget([min, max])
}

/// A dimension across: arrows at both ends, extension lines down from their y to just past
/// the line, the figure under it.
pub(super) fn horizontal_dimension(canvas: &Canvas, dimension: &Dimension) {
    let Dimension { from, to, .. } = *dimension;
    let y = from[1];
    line(canvas, from, to, DIMENSION_PEN);
    arrow(canvas, from, PI);
    arrow(canvas, to, 0.0);
    for (end, start) in [from[0], to[0]].into_iter().zip(dimension.extension_from) {
        line(
            canvas,
            [end, start],
            [end, y + EXTENSION_OVERSHOOT],
            EXTENSION_PEN,
        );
    }
    let at = [(from[0] + to[0]) / 2.0, y + LABEL_BELOW];
    let label = write(canvas, at, dimension.label, &DIMENSION_LABEL);
    let bounds = union(line_and_arrows(canvas, from, to), label);
    canvas.mark(Part::Dimension, bounds);
}

/// A dimension down: arrows at both ends, extension lines across from their x to just past
/// the line, the figure beside it, reading upwards.
pub(super) fn vertical_dimension(canvas: &Canvas, dimension: &Dimension) {
    let Dimension { from, to, .. } = *dimension;
    let x = from[0];
    line(canvas, from, to, DIMENSION_PEN);
    arrow(canvas, from, -FRAC_PI_2);
    arrow(canvas, to, FRAC_PI_2);
    for (end, start) in [from[1], to[1]].into_iter().zip(dimension.extension_from) {
        line(
            canvas,
            [start, end],
            [x + EXTENSION_OVERSHOOT, end],
            EXTENSION_PEN,
        );
    }
    let _ = canvas.save();
    canvas.translate(x + LABEL_BESIDE, (from[1] + to[1]) / 2.0);
    canvas.rotate(-FRAC_PI_2);
    let label = write(
        canvas,
        [0.0, -LABEL_LIFT],
        dimension.label,
        &DIMENSION_LABEL,
    );
    let _ = canvas.restore();
    let bounds = union(line_and_arrows(canvas, from, to), label);
    canvas.mark(Part::Dimension, bounds);
}

/// Writes `text` with its baseline at `at`, each letter spread by the lettering's spacing,
/// and gives the box it covers in the widget.
pub(super) fn write(canvas: &Canvas, at: [f64; 2], text: &str, lettering: &Lettering) -> Bounds {
    canvas.select_font_face(FONT, cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    canvas.set_font_size(lettering.size);
    let gap = lettering.spacing_em * lettering.size;
    let letters: Vec<String> = text.chars().map(String::from).collect();
    let steps: Vec<f64> = letters
        .iter()
        .map(|letter| advance(canvas, letter) + gap)
        .collect();
    let width = steps.iter().sum::<f64>() - gap;
    let start = match lettering.anchor {
        Anchor::Start => at[0],
        Anchor::Middle => at[0] - (width + gap) / 2.0,
    };
    let [red, green, blue] = lettering.colour;
    canvas.set_source_rgb(red, green, blue);
    let mut x = start;
    for (letter, step) in letters.iter().zip(&steps) {
        canvas.move_to(x, at[1]);
        let _ = canvas.show_text(letter);
        x += step;
    }
    let [top, bottom] = vertical_reach(canvas, text);
    canvas.on_widget([[start, at[1] + top], [start + width, at[1] + bottom]])
}

fn advance(context: &cairo::Context, letter: &str) -> f64 {
    context
        .text_extents(letter)
        .map_or(0.0, |extents| extents.x_advance())
}

/// How far `text`'s ink reaches above (negative) and below its baseline.
fn vertical_reach(context: &cairo::Context, text: &str) -> [f64; 2] {
    context.text_extents(text).map_or([0.0; 2], |extents| {
        [extents.y_bearing(), extents.y_bearing() + extents.height()]
    })
}
