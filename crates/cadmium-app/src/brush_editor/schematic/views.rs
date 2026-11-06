//! One view of the tip: the ferrule (crimped) and the cut handle above the roots, the hair
//! bundle and its guide lines below, a centre line, and the view's caption.

use cadmium_studio::{TipGeometry, TipView};

use super::canvas::{Bounds, Canvas, Part};
use super::pen::{self, CAPTION, CENTRE_DASH, INK, PAPER, Pen, THIN};
use super::sheet::{FRONT_X, SIDE_X, Sheet};
use crate::ink::{Fit, polyline, trace};

const FERRULE_PEN: Pen = Pen::new(INK, 1.2);
const OUTLINE_PEN: Pen = Pen::new(INK, 1.4);
const HAIR_PEN: Pen = Pen::new(THIN, 0.45);
const CRIMP_PEN: Pen = Pen::new(THIN, 0.5);
const CUT_PEN: Pen = Pen::new(THIN, 0.8);
const CENTRE_PEN: Pen = Pen::new(THIN, 0.5);
const OUTLINE_FILL_ALPHA: f64 = 0.05;
const CUT_OVERHANG: f64 = 3.0;
const CUT_WAVE: f64 = 4.0;
const CENTRE_ABOVE: f64 = 6.0;
const CENTRE_BELOW: f64 = 10.0;
const CAPTION_ABOVE: f64 = 12.0;
/// A ferrule mouth narrower than this (a head thinner than 0.2) is drawn wider by
/// `THIN_MOUTH_EXTRA` sheet units, fading out up its neck, so it still reads (the mockup's).
const THIN_MOUTH: f32 = 0.2 * 1.035;
const THIN_MOUTH_EXTRA: f64 = 1.5;

/// A view placed on the sheet.
pub(super) struct Placed<'a> {
    view: &'a TipView,
    fit: Fit,
    caption: &'static str,
    /// What the view is, for the tests' record of where it landed.
    part: Part,
    /// The centre hair's length, in root half-widths.
    length: f64,
    /// Sheet units added to a thin ferrule's mouth.
    mouth_extra: f64,
}

impl<'a> Placed<'a> {
    pub(super) fn front(tip: &'a TipGeometry, sheet: &Sheet) -> Placed<'a> {
        Placed::new(&tip.front, tip, sheet, FRONT_X, ("FRONT", Part::FrontView))
    }

    pub(super) fn side(tip: &'a TipGeometry, sheet: &Sheet) -> Placed<'a> {
        Placed::new(&tip.side, tip, sheet, SIDE_X, ("SIDE", Part::SideView))
    }

    fn new(
        view: &'a TipView,
        tip: &TipGeometry,
        sheet: &Sheet,
        x: f64,
        (caption, part): (&'static str, Part),
    ) -> Placed<'a> {
        let thin = view.ferrule.mouth < THIN_MOUTH;
        Placed {
            view,
            fit: Fit {
                scale: sheet.scale,
                origin: [x, sheet.roots_y],
            },
            caption,
            part,
            length: f64::from(tip.dimensions.length),
            mouth_extra: if thin { THIN_MOUTH_EXTRA } else { 0.0 },
        }
    }
}

pub(super) fn draw_view(canvas: &Canvas, placed: &Placed) {
    canvas.part(placed.part, || {
        ferrule(canvas, placed);
        handle(canvas, placed);
        bundle(canvas, placed);
        centre_line(canvas, placed);
        canvas.covers(|| covered(placed));
    });
}

fn ferrule(canvas: &Canvas, placed: &Placed) {
    trace(canvas, &placed.fit, &widened(placed));
    pen::fill_and_stroke(canvas, PAPER, 1.0, FERRULE_PEN);
    let ferrule = &placed.view.ferrule;
    let x = placed.fit.origin[0];
    let half = f64::from(ferrule.mouth) * placed.fit.scale + placed.mouth_extra;
    for crimp_y in ferrule.crimp {
        let (_, y) = placed.fit.place([0.0, crimp_y]);
        pen::line(canvas, [x - half, y], [x + half, y], CRIMP_PEN);
    }
}

/// The ferrule's outline, a thin one widened at its mouth and less so up its neck.
fn widened(placed: &Placed) -> Vec<[f32; 2]> {
    let ferrule = &placed.view.ferrule;
    let radius = ferrule.handle[0][0];
    let extra = (placed.mouth_extra / placed.fit.scale) as f32;
    if extra <= 0.0 || radius <= ferrule.mouth {
        return ferrule.outline.clone();
    }
    let neck = radius - ferrule.mouth;
    ferrule
        .outline
        .iter()
        .map(|&[x, y]| {
            let share = ((radius - x.abs()) / neck).clamp(0.0, 1.0);
            [x + x.signum() * extra * share, y]
        })
        .collect()
}

fn handle(canvas: &Canvas, placed: &Placed) {
    let [bottom, top] = placed
        .view
        .ferrule
        .handle
        .map(|point| placed.fit.place(point));
    let x = placed.fit.origin[0];
    for side in [-1.0, 1.0] {
        let edge = |(px, py): (f64, f64)| [x + side * (px - x), py];
        pen::line(canvas, edge(bottom), edge(top), FERRULE_PEN);
    }
    cut_mark(canvas, [x, top.1], top.0 - x);
}

/// The wavy line where the drawing cuts the handle, `half` either side of `centre`.
fn cut_mark(canvas: &Canvas, centre: [f64; 2], half: f64) {
    let span = half + CUT_OVERHANG;
    let [x, y] = centre;
    let start = [x - span, y];
    let end = [x + span, y];
    pen::ink(canvas, CUT_PEN);
    canvas.move_to(start[0], start[1]);
    pen::quad_to(canvas, start, [x - span / 2.0, y - CUT_WAVE], centre);
    pen::quad_to(canvas, centre, [x + span / 2.0, y + CUT_WAVE], end);
    let _ = canvas.stroke();
}

fn bundle(canvas: &Canvas, placed: &Placed) {
    trace(canvas, &placed.fit, &placed.view.outline);
    pen::fill_and_stroke(canvas, INK, OUTLINE_FILL_ALPHA, OUTLINE_PEN);
    pen::ink(canvas, HAIR_PEN);
    for hair in &placed.view.hairs {
        polyline(canvas, &placed.fit, hair);
    }
    let _ = canvas.stroke();
}

fn centre_line(canvas: &Canvas, placed: &Placed) {
    let x = placed.fit.origin[0];
    let (_, top) = placed.fit.place(placed.view.ferrule.handle[1]);
    let bottom = placed.fit.origin[1] + placed.length * placed.fit.scale + CENTRE_BELOW;
    pen::dashed(
        canvas,
        [x, top - CENTRE_ABOVE],
        [x, bottom],
        &CENTRE_DASH,
        CENTRE_PEN,
    );
    let at = [x, top - CAPTION_ABOVE];
    canvas.part(Part::Caption, || {
        pen::write(canvas, at, placed.caption, &CAPTION)
    });
}

/// The box the view covers: the bundle, the ferrule widened at a thin mouth, the handle and
/// its cut mark.
fn covered(placed: &Placed) -> Bounds {
    let [min, max] = placed.view.extent();
    let (left, top) = placed.fit.place(min);
    let (right, bottom) = placed.fit.place(max);
    let x = placed.fit.origin[0];
    let (cut_x, _) = placed.fit.place(placed.view.ferrule.handle[1]);
    let reach = (cut_x - x + CUT_OVERHANG).max(x - left + placed.mouth_extra);
    let reach = reach.max(right - x + placed.mouth_extra);
    [[x - reach, top - CUT_WAVE / 2.0], [x + reach, bottom]]
}
