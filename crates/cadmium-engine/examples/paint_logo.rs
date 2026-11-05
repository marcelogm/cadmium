//! Paints the Cadmium logo with the engine's own brush and saves it for the README.
//!
//! cargo run --release -p cadmium-engine --example paint_logo -- <out dir>
//!
//! Writes `logo.cadmium` (open it in the app) and `logo.png`.

use std::f64::consts::{PI, TAU};
use std::path::PathBuf;

use cadmium_engine::canvas::{CanvasSpec, Primer, Weave};
use cadmium_engine::document::{export_png, save};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

const SIZE: u32 = 1024;
const CENTER: (f64, f64) = (512.0, 452.0);
const RADIUS: f64 = 400.0;
const INNER_RADIUS: f64 = 240.0;
const SLAB_DEPTH: f64 = 80.0;
const SAMPLE_STEP_PX: f64 = 3.0;
const SAMPLES_PER_FRAME: usize = 4;

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

type Point = (f64, f64);

struct Painter {
    gpu: GpuContext,
    session: PaintSession,
}

impl Painter {
    fn load(&mut self, recipe: &[(usize, f32)], diameter_px: f32) {
        self.session.brush.recipe = recipe.to_vec();
        self.session.brush.geometry.max_diameter_px = diameter_px;
        self.session.brush.load = 1.0;
        self.session.reload_brush(&self.gpu);
    }

    fn frame(&mut self) {
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        self.session.encode_frame(&self.gpu, &mut encoder);
        self.gpu.queue.submit([encoder.finish()]);
    }

    /// One stroke through `path`, pressing harder in the middle like a hand does.
    fn stroke(&mut self, path: &[Point]) {
        let samples = resample(path);
        let last = samples.len().saturating_sub(1).max(1) as f64;
        for (i, &(x, y)) in samples.iter().enumerate() {
            let t = i as f64 / last;
            let sample = StrokeSample {
                x,
                y,
                pressure: (0.55 + 0.45 * (t * PI).sin()) as f32,
                tilt_x_deg: 20.0,
                tilt_y_deg: 0.0,
                time_ms: i as u32 * 8,
            };
            if i == 0 {
                self.session.begin_stroke(&self.gpu, sample);
            } else {
                self.session.add_samples(&[sample]);
            }
            if i % SAMPLES_PER_FRAME == 0 {
                self.frame();
            }
        }
        self.session.end_stroke();
        self.frame();
    }

    fn dry(&mut self) {
        self.session.dry(&self.gpu);
        self.frame();
    }
}

fn resample(path: &[Point]) -> Vec<Point> {
    let mut points = Vec::new();
    for pair in path.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        let steps = ((x1 - x0).hypot(y1 - y0) / SAMPLE_STEP_PX).ceil().max(1.0) as usize;
        points.extend((0..steps).map(|s| {
            let t = s as f64 / steps as f64;
            (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
        }));
    }
    points.extend(path.last());
    points
}

fn hexagon(center: Point, radius: f64) -> [Point; 6] {
    std::array::from_fn(|i| {
        let angle = -PI / 2.0 + i as f64 * PI / 3.0;
        (center.0 + radius * angle.cos(), center.1 + radius * angle.sin())
    })
}

/// Horizontal extent of a pointy-top hexagon at height `y`, if the row crosses it.
fn row_span(center: Point, radius: f64, y: f64) -> Option<(f64, f64)> {
    let dy = (y - center.1).abs();
    let half_width = radius * 3f64.sqrt() / 2.0;
    if dy > radius {
        return None;
    }
    let width = if dy <= radius / 2.0 {
        half_width
    } else {
        half_width * (radius - dy) / (radius / 2.0)
    };
    Some((center.0 - width, center.0 + width))
}

/// Fills one side of the hexagon with back-and-forth rows, reloading for every row.
fn fill_half(painter: &mut Painter, center: Point, left: bool, recipe: &[(usize, f32)]) {
    let diameter: f32 = 64.0;
    let mut y = center.1 - RADIUS + 20.0;
    let mut row = 0;
    while y < center.1 + RADIUS - 12.0 {
        if let Some((x0, x1)) = row_span(center, RADIUS - 14.0, y) {
            let inset = f64::from(diameter) * 0.35;
            let (x0, x1) = (x0 + inset, x1 - inset);
            if x1 - x0 < f64::from(diameter) {
                y += f64::from(diameter) * 0.55;
                continue;
            }
            let (from, to) = if left { (x0, center.0 + 6.0) } else { (center.0 - 6.0, x1) };
            let wobble = if row % 2 == 0 { 6.0 } else { -6.0 };
            let path = [(from, y), ((from + to) / 2.0, y + wobble), (to, y)];
            painter.load(recipe, diameter);
            painter.stroke(&path);
        }
        y += f64::from(diameter) * 0.55;
        row += 1;
    }
}

fn paint_slab(painter: &mut Painter) {
    let deep = [(YELLOW, 3.0), (RED, 0.55), (BLACK, 0.1)];
    let lowered = (CENTER.0, CENTER.1 + SLAB_DEPTH);
    fill_half(painter, lowered, true, &deep);
    fill_half(painter, lowered, false, &deep);
    painter.dry();
}

fn paint_faces(painter: &mut Painter) {
    fill_half(painter, CENTER, true, &[(YELLOW, 1.0), (WHITE, 0.04)]);
    fill_half(painter, CENTER, false, &[(YELLOW, 3.0), (RED, 0.22)]);
    painter.dry();
}

fn paint_bonds(painter: &mut Painter) {
    let inner = hexagon(CENTER, INNER_RADIUS);
    for i in 0..6 {
        painter.load(&[(YELLOW, 1.0), (WHITE, 3.0)], 30.0);
        painter.stroke(&[inner[i], inner[(i + 1) % 6]]);
    }
}

/// A small spiral that covers a disk, the way a round brush dabs an atom in.
fn paint_disk(painter: &mut Painter, center: Point, radius: f64, recipe: &[(usize, f32)]) {
    painter.load(recipe, (radius * 0.9) as f32);
    let turns = 2.5;
    let path: Vec<Point> = (0..=90)
        .map(|i| {
            let t = f64::from(i) / 90.0;
            let r = radius * 0.55 * (1.0 - t);
            let angle = t * turns * TAU;
            (center.0 + r * angle.cos(), center.1 + r * angle.sin())
        })
        .collect();
    painter.stroke(&path);
}

fn paint_atoms(painter: &mut Painter) {
    let dark = [(BLACK, 2.0), (BLUE, 1.0), (RED, 0.3)];
    for atom in hexagon(CENTER, INNER_RADIUS) {
        paint_disk(painter, atom, 52.0, &dark);
    }
    paint_disk(painter, CENTER, 60.0, &[(WHITE, 3.0), (BLACK, 0.5), (BLUE, 0.35)]);
    paint_disk(painter, (CENTER.0 - 16.0, CENTER.1 - 16.0), 18.0, &[(WHITE, 1.0)]);
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
    let mut painter = Painter { gpu, session };

    paint_slab(&mut painter);
    paint_faces(&mut painter);
    paint_bonds(&mut painter);
    paint_atoms(&mut painter);

    save(&painter.gpu, &mut painter.session, &out.join("logo.cadmium")).expect("save");
    export_png(&painter.gpu, &mut painter.session, &out.join("logo.png")).expect("export");
}
