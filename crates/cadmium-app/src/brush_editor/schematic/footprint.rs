//! The footprint: the tip seen from below, the ferrule's mouth dashed round it, filled in blue.

use std::f64::consts::TAU;

use cadmium_studio::TipGeometry;

use super::canvas::{Canvas, Part, hull};
use super::dimensions::{MOUTH, SHORT_DASH, THICKNESS_SLACK};
use super::pen::{self, CAPTION, CENTRE_DASH, DIMENSION, Pen, THIN};
use super::sheet::{FOOTPRINT_X, Sheet};
use crate::ink::{Fit, trace};

/// The footprint's caption.
pub(super) const CAPTION_TEXT: &str = "FOOTPRINT";
const RING_PEN: Pen = Pen::new(THIN, 0.7);
const FOOTPRINT_PEN: Pen = Pen::new(DIMENSION, 1.0);
const CENTRE_PEN: Pen = Pen::new(THIN, 0.5);
const FOOTPRINT_FILL_ALPHA: f64 = 0.18;
const RING_EXTRA: f64 = 1.0;
const MIN_CENTRE_REACH: f64 = 1.3;
const CENTRE_OVERSHOOT: f64 = 8.0;

pub(super) fn draw(canvas: &Canvas, tip: &TipGeometry, sheet: &Sheet) {
    let at = [FOOTPRINT_X, sheet.footprint_caption_y];
    canvas.part(Part::Caption, || {
        pen::write(canvas, at, CAPTION_TEXT, &CAPTION)
    });
    canvas.part(Part::Footprint, || footprint(canvas, tip, sheet));
}

fn footprint(canvas: &Canvas, tip: &TipGeometry, sheet: &Sheet) {
    let (y, scale) = (sheet.footprint_y, sheet.footprint_scale);
    let thickness = f64::from(tip.dimensions.thickness);
    let radii = [
        MOUTH * scale,
        thickness * THICKNESS_SLACK * scale + RING_EXTRA,
    ];
    mouth_ring(canvas, [FOOTPRINT_X, y], radii);
    let fit = Fit {
        scale,
        origin: [FOOTPRINT_X, y],
    };
    trace(canvas, &fit, &tip.footprint);
    pen::fill_and_stroke(canvas, DIMENSION, FOOTPRINT_FILL_ALPHA, FOOTPRINT_PEN);
    let [min, max] = tip.footprint_bounds();
    let reach = f64::from(min[0].abs().max(max[0])).max(MIN_CENTRE_REACH);
    let from = [FOOTPRINT_X - MIN_CENTRE_REACH * scale - CENTRE_OVERSHOOT, y];
    let to = [FOOTPRINT_X + reach * scale + CENTRE_OVERSHOOT, y];
    pen::dashed(canvas, from, to, &CENTRE_DASH, CENTRE_PEN);
    canvas.covers(|| {
        let shape = [min, max].map(|corner| {
            let (corner_x, corner_y) = fit.place(corner);
            [corner_x, corner_y]
        });
        let ring = [
            [FOOTPRINT_X - radii[0], y - radii[1]],
            [FOOTPRINT_X + radii[0], y + radii[1]],
        ];
        hull(&[shape[0], shape[1], ring[0], ring[1]])
    });
}

/// A dashed ellipse of `radii` round `centre`, its line width unscaled.
fn mouth_ring(canvas: &Canvas, centre: [f64; 2], radii: [f64; 2]) {
    let _ = canvas.save();
    canvas.translate(centre[0], centre[1]);
    canvas.scale(radii[0], radii[1]);
    canvas.new_path();
    canvas.arc(0.0, 0.0, 1.0, 0.0, TAU);
    let _ = canvas.restore();
    canvas.set_dash(&SHORT_DASH, 0.0);
    pen::ink(canvas, RING_PEN);
    let _ = canvas.stroke();
    canvas.set_dash(&[], 0.0);
}
