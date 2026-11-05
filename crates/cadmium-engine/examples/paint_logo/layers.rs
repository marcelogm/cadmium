//! A layer of paint over one shape: strokes laid in lanes so no canvas shows through,
//! then the outline cut in with short strokes along each edge.

use std::f64::consts::PI;

use crate::hand::{Gesture, Point, Rng, Tint};
use crate::hexagon::Polygon;
use crate::painter::{Brush, Painter};

/// How one layer is laid: the brush, which way the strokes lean, how long they are, how
/// tightly they sit, and how the mix shifts from the top of the shape to its bottom.
#[derive(Clone, Copy)]
pub struct Layer {
    pub brush: Brush,
    pub angle: f64,
    pub angle_spread: f64,
    pub length: (f64, f64),
    /// Gap between lanes, in brush widths.
    pub lane_gap: f64,
    /// Advance along a lane, in stroke lengths; below 1 the strokes overlap.
    pub step: f64,
    pub tint: Tint,
    pub tint_spread: f64,
    /// Chance of dipping again before a stroke instead of going on with what is left.
    pub reload: f64,
    /// Chance that a stroke is painted at all, for sparse accents.
    pub keep: f64,
    /// Lay the strokes in a loose, shuffled order instead of lane by lane.
    pub shuffled: bool,
}

impl Layer {
    fn width(&self) -> f64 {
        f64::from(self.brush.diameter)
    }

    fn dip(&self, painter: &mut Painter, shape: &Polygon, at: Point) {
        let (top, bottom) = shape.top_and_bottom();
        let t = (at.1 - top) / (bottom - top) + painter.rng.spread(self.tint_spread);
        painter.load(self.brush, &self.tint.at(t));
    }
}

/// Covers `shape` with the layer's strokes, keeping them a little off its outline.
pub fn paint(painter: &mut Painter, shape: &Polygon, layer: &Layer) {
    let mut strokes = lanes(&mut painter.rng, shape, layer);
    if layer.shuffled {
        painter.rng.shuffle(&mut strokes);
        let band = layer.length.1 * 1.5;
        strokes.sort_by_key(|g| (g.middle.1 / band) as i64);
    }
    let margin = layer.width() * 0.45;
    let mut loaded = false;
    for gesture in strokes {
        if !painter.rng.chance(layer.keep) {
            continue;
        }
        if !loaded || painter.rng.chance(layer.reload) {
            layer.dip(painter, shape, gesture.middle);
            loaded = true;
        }
        painter.gesture(&gesture, |p| shape.depth(p) >= margin);
    }
}

fn lanes(rng: &mut Rng, shape: &Polygon, layer: &Layer) -> Vec<Gesture> {
    let across = layer.angle + PI / 2.0;
    let (first, last) = shape.extent(across);
    let gap = layer.lane_gap * layer.width();
    let mut offset = first + gap * rng.range(0.2, 0.6);
    let mut strokes = Vec::new();
    while offset < last {
        strokes.extend(lane(rng, shape, layer, offset));
        offset += gap * rng.range(0.85, 1.15);
    }
    strokes
}

fn lane(rng: &mut Rng, shape: &Polygon, layer: &Layer, offset: f64) -> Vec<Gesture> {
    let (along_x, along_y) = (layer.angle.cos(), layer.angle.sin());
    let (start, end) = shape.extent(layer.angle);
    let mut at = start - rng.range(0.0, layer.length.1);
    let mut strokes = Vec::new();
    while at < end {
        let length = rng.range(layer.length.0, layer.length.1);
        let mid = at + length / 2.0;
        let middle = (
            mid * along_x - offset * along_y,
            mid * along_y + offset * along_x,
        );
        let flip = if rng.chance(0.3) { PI } else { 0.0 };
        strokes.push(Gesture {
            middle,
            angle: layer.angle + rng.spread(layer.angle_spread) + flip,
            length,
            bend: rng.spread(0.06),
        });
        at += length * layer.step * rng.range(0.8, 1.2);
    }
    strokes.retain(|g| shape.depth(g.middle) > -g.length / 2.0);
    strokes
}

/// Short overlapping strokes along every edge, the way a painter cuts in an outline.
pub fn cut_in(painter: &mut Painter, shape: &Polygon, layer: &Layer) {
    for edge in shape.edges() {
        cut_in_edge(painter, shape, layer, edge);
    }
}

fn cut_in_edge(painter: &mut Painter, shape: &Polygon, layer: &Layer, (a, b): (Point, Point)) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx.hypot(dy);
    let inward = shape.inward((a, b));
    let inset = layer.width() * 0.5;
    let margin = layer.width() * 0.3;
    let mut along = -painter.rng.range(0.0, layer.length.0 * 0.5);
    while along < length {
        let piece = painter.rng.range(layer.length.0, layer.length.1);
        let t = (along + piece / 2.0) / length;
        let depth = inset + painter.rng.spread(inset * 0.12);
        let middle = (
            a.0 + dx * t + inward.0 * depth,
            a.1 + dy * t + inward.1 * depth,
        );
        layer.dip(painter, shape, middle);
        let gesture = Gesture {
            middle,
            angle: dy.atan2(dx) + painter.rng.spread(0.05),
            length: piece,
            bend: painter.rng.spread(0.02),
        };
        painter.gesture(&gesture, |p| shape.depth(p) >= margin);
        along += piece * painter.rng.range(0.6, 0.8);
    }
}
