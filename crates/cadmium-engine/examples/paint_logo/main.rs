//! Paints the Cadmium logo with the engine's own bristle brush and saves it for the README.
//!
//! cargo run --release -p cadmium-engine --example paint_logo -- <out dir>
//!
//! Writes `logo.cadmium` (open it in the app) and `logo.png`. It is painted the way a
//! painter would: a thin underpainting that covers the canvas, left to dry; a loaded body
//! layer of short strokes over it, dried; then the bonds and the atoms. The strokes
//! are seeded at random, so it looks hand-made but comes out the same on every run.

mod hand;
mod hexagon;
mod layers;
mod painter;

use std::f64::consts::{PI, TAU};
use std::path::PathBuf;

use cadmium_engine::brush::BrushShape;
use cadmium_engine::canvas::{CanvasSpec, Primer, Weave};
use cadmium_engine::document::{export_png, save};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;

use hand::{Gesture, Point, Rng, Tint};
use hexagon::{Hexagon, Polygon, Side};
use layers::{Layer, cut_in, paint};
use painter::{Brush, Painter};

const SIZE: u32 = 1024;
const SEED: u64 = 0xCAD_0003;
const CENTER: Point = (512.0, 452.0);
const RADIUS: f64 = 386.0;
const INNER_RADIUS: f64 = 240.0;
const SLAB_DEPTH: f64 = 80.0;

const YELLOW: usize = 0;
const RED: usize = 1;
const WHITE: usize = 2;
const BLACK: usize = 3;
const BLUE: usize = 4;
const PALETTE: [&str; 5] = [
    "cadmium_yellow",
    "cadmium_red",
    "titanium_white",
    "ivory_black",
    "ultramarine_blue",
];

const LEMON: Tint = Tint {
    from: &[(YELLOW, 1.0), (WHITE, 0.16)],
    to: &[(YELLOW, 1.0), (RED, 0.004)],
};
const ORANGE: Tint = Tint {
    from: &[(YELLOW, 3.0), (RED, 0.15), (WHITE, 0.08)],
    to: &[(YELLOW, 3.0), (RED, 0.32)],
};
const SLAB_LIT: Tint = Tint {
    from: &[(YELLOW, 3.0), (RED, 0.5), (BLACK, 0.08)],
    to: &[(YELLOW, 3.0), (RED, 0.65), (BLACK, 0.15)],
};
const SLAB_SHADE: Tint = Tint {
    from: &[(YELLOW, 3.0), (RED, 0.65), (BLACK, 0.16)],
    to: &[(YELLOW, 3.0), (RED, 0.8), (BLACK, 0.26)],
};

/// Thin paint, wide flat brush, lanes overlapping enough that no canvas is left bare.
fn underpainting(angle: f64, tint: Tint) -> Layer {
    Layer {
        brush: Brush {
            shape: BrushShape::Flat,
            diameter: 56.0,
            load: 0.9,
            medium: 0.35,
        },
        angle,
        angle_spread: 0.12,
        length: (60.0, 110.0),
        lane_gap: 0.45,
        step: 0.6,
        tint,
        tint_spread: 0.1,
        reload: 1.0,
        keep: 1.0,
        shuffled: false,
    }
}

/// Short, loaded strokes in a loose order, leaning a little off the underpainting.
fn body(angle: f64, tint: Tint) -> Layer {
    Layer {
        brush: Brush {
            shape: BrushShape::Flat,
            diameter: 44.0,
            load: 1.0,
            medium: 0.08,
        },
        angle: angle + 0.2,
        angle_spread: 0.25,
        length: (40.0, 85.0),
        lane_gap: 0.8,
        step: 0.85,
        tint_spread: 0.3,
        reload: 0.85,
        shuffled: true,
        ..underpainting(angle, tint)
    }
}

fn face_hex() -> Hexagon {
    Hexagon {
        center: CENTER,
        radius: RADIUS,
    }
}

/// Lays each layer over its piece, cuts in the outlines, and lets it all dry.
fn layer_and_dry(painter: &mut Painter, pieces: &[(Polygon, Layer)]) {
    for (shape, layer) in pieces {
        paint(painter, shape, layer);
        cut_in(painter, shape, layer);
    }
    painter.dry();
}

fn paint_slab(painter: &mut Painter) {
    let hex = face_hex();
    let pieces = |make: fn(f64, Tint) -> Layer| {
        [
            (
                hex.slab_side(Side::Left, SLAB_DEPTH, 30.0),
                make(PI / 6.0, SLAB_LIT),
            ),
            (
                hex.slab_side(Side::Right, SLAB_DEPTH, 30.0),
                make(-PI / 6.0, SLAB_SHADE),
            ),
        ]
    };
    layer_and_dry(painter, &pieces(underpainting));
    layer_and_dry(painter, &pieces(body));
}

fn paint_faces(painter: &mut Painter) {
    let hex = face_hex();
    let pieces = |make: fn(f64, Tint) -> Layer| {
        [
            (hex.half(Side::Left), make(PI / 3.0, LEMON)),
            (hex.half(Side::Right), make(2.0 * PI / 3.0, ORANGE)),
        ]
    };
    layer_and_dry(painter, &pieces(underpainting));
    layer_and_dry(painter, &pieces(body));
}

/// Each bond in two overlapping pulls of a round brush, never quite on the line.
fn paint_bonds(painter: &mut Painter) {
    let bond = Brush {
        shape: BrushShape::Round,
        diameter: 28.0,
        load: 1.0,
        medium: 0.05,
    };
    let ring = Hexagon {
        center: CENTER,
        radius: INNER_RADIUS,
    };
    for (a, b) in ring.whole().edges() {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        for t in [0.28, 0.72] {
            painter.load(bond, &[(YELLOW, 1.0), (WHITE, 3.0)]);
            let gesture = Gesture {
                middle: (a.0 + dx * t, a.1 + dy * t + painter.rng.spread(2.0)),
                angle: dy.atan2(dx) + painter.rng.spread(0.03),
                length: dx.hypot(dy) * painter.rng.range(0.55, 0.62),
                bend: painter.rng.spread(0.03),
            };
            painter.gesture(&gesture, |_| true);
        }
    }
    painter.dry();
}

/// A loaded round brush twisted inward from the rim, twice from opposite sides so the
/// paint the brush drops where it lands evens out; never quite a circle.
fn paint_atom(painter: &mut Painter, center: Point, radius: f64, recipe: &[(usize, f32)]) {
    let brush = Brush {
        shape: BrushShape::Round,
        diameter: (radius * 0.9) as f32,
        load: 1.0,
        medium: 0.05,
    };
    let phase = painter.rng.range(0.0, TAU);
    for start in [phase, phase + PI] {
        painter.load(brush, recipe);
        let path = spiral(&mut painter.rng, center, radius, start);
        painter.stroke_upright(&path);
    }
}

fn spiral(rng: &mut Rng, center: Point, radius: f64, phase: f64) -> Vec<Point> {
    let turns = rng.range(1.2, 1.6);
    let squash = rng.range(0.94, 1.06);
    let mut wander = (0.0, 0.0);
    (0..=60)
        .map(|i| {
            let t = f64::from(i) / 60.0;
            wander.0 = (wander.0 + rng.spread(0.7)) * 0.9;
            wander.1 = (wander.1 + rng.spread(0.7)) * 0.9;
            let r = radius * 0.55 * (1.0 - t * 0.9);
            let angle = phase + t * turns * TAU;
            (
                center.0 + r * angle.cos() * squash + wander.0,
                center.1 + r * angle.sin() / squash + wander.1,
            )
        })
        .collect()
}

fn paint_atoms(painter: &mut Painter) {
    let dark = [(BLACK, 2.0), (BLUE, 1.0), (RED, 0.3)];
    let ring = Hexagon {
        center: CENTER,
        radius: INNER_RADIUS,
    };
    for atom in ring.corners() {
        paint_atom(painter, atom, 52.0, &dark);
    }
    paint_atom(
        painter,
        CENTER,
        66.0,
        &[(WHITE, 3.0), (BLACK, 0.5), (BLUE, 0.35)],
    );
    painter.dry();
    let glint = Brush {
        shape: BrushShape::Round,
        diameter: 18.0,
        load: 1.0,
        medium: 0.05,
    };
    painter.load(glint, &[(WHITE, 1.0)]);
    let stroke = Gesture {
        middle: (CENTER.0 - 18.0, CENTER.1 - 20.0),
        angle: -PI / 4.0,
        length: 30.0,
        bend: 0.25,
    };
    painter.gesture(&stroke, |_| true);
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    let gpu = GpuContext::new().expect("a Vulkan GPU");
    let spec = CanvasSpec {
        width: SIZE,
        height: SIZE,
        primer: Primer::WhiteGesso,
        weave: Weave::Linen,
        pigment_slots: PALETTE.iter().map(|id| id.to_string()).collect(),
    };
    let session = PaintSession::new(&gpu, spec).expect("canvas");
    let mut painter = Painter::new(gpu, session, SEED);

    paint_slab(&mut painter);
    paint_faces(&mut painter);
    paint_bonds(&mut painter);
    paint_atoms(&mut painter);

    save(
        &painter.gpu,
        &mut painter.session,
        &out.join("logo.cadmium"),
    )
    .expect("save");
    export_png(&painter.gpu, &mut painter.session, &out.join("logo.png")).expect("export");
}
