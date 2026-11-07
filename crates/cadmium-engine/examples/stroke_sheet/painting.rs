use std::f64::consts::TAU;

use cadmium_engine::canvas::{CanvasSpec, Primer, Weave};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::present::{PresentRequest, ViewTransform, readback_rgba8};
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::{StrokeSample, Tool};

use crate::scenarios::Cell;

pub(crate) const SHEET_WIDTH_PX: u32 = 1100;
pub(crate) const SHEET_HEIGHT_PX: u32 = 360;
const PIGMENTS: [&str; 3] = ["ultramarine_blue", "cadmium_yellow", "titanium_white"];
const BLUE: usize = 0;
const YELLOW: usize = 1;
const SAMPLES_PER_SECOND: f64 = 120.0;
const FRAMES_PER_SECOND: f64 = 60.0;
const SAMPLES_PER_FRAME: usize = (SAMPLES_PER_SECOND / FRAMES_PER_SECOND) as usize;
const STRIPE_XS_PX: [f64; 4] = [220.0, 440.0, 660.0, 880.0];
const STRIPE_DIAMETER_PX: f32 = 70.0;
const STRIPE_SPEED_PX_PER_SECOND: f64 = 800.0;
const STROKE_MARGIN_PX: f64 = 90.0;
const STROKE_AMPLITUDE_PX: f64 = 60.0;
const STROKE_TILT_DEG: f32 = 20.0;
const PRESSURE_START: f32 = 0.3;
const PRESSURE_PEAK: f32 = 1.0;
const PRESSURE_END: f32 = 0.5;
const MAX_SETTLE_FRAMES: usize = 2000;
const PATH_STEPS: usize = 4000;

#[derive(Clone, Copy)]
pub(crate) enum Candidate {
    Reference,
    FrameLimit(usize),
    Spacing(f32),
}

impl Candidate {
    pub(crate) fn parse(text: &str) -> Option<Candidate> {
        let (kind, value) = text.split_once('=')?;
        match kind {
            "limit" => value.parse().ok().map(Candidate::FrameLimit),
            "spacing" => value.parse().ok().map(Candidate::Spacing),
            _ => None,
        }
    }
}

struct Painter<'gpu> {
    gpu: &'gpu GpuContext,
    session: PaintSession,
    frame_index: u32,
}

impl Painter<'_> {
    fn frame(&mut self) {
        let clock_ms = f64::from(self.frame_index) * 1000.0 / FRAMES_PER_SECOND;
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        self.session.encode_frame(self.gpu, clock_ms, &mut encoder);
        self.gpu.queue.submit([encoder.finish()]);
        self.frame_index += 1;
    }

    fn clock_ms(&self) -> f64 {
        f64::from(self.frame_index) * 1000.0 / FRAMES_PER_SECOND
    }

    fn stroke(&mut self, samples: &[StrokeSample]) {
        let Some((&first, rest)) = samples.split_first() else {
            return;
        };
        self.session.begin_stroke(first);
        self.frame();
        for chunk in rest.chunks(SAMPLES_PER_FRAME) {
            self.session.add_samples(chunk);
            self.frame();
        }
        self.session.end_stroke();
        self.frame();
        let mut settle_frames = 0;
        while self.busy() && settle_frames < MAX_SETTLE_FRAMES {
            self.frame();
            settle_frames += 1;
        }
    }

    fn busy(&self) -> bool {
        self.session.has_pending_work() || self.session.bristles_settling(self.clock_ms())
    }

    fn take_up(&mut self, tool: Tool, diameter_px: f32, load: f32, slot: usize) {
        let brush = &mut self.session.brush;
        brush.tool = tool;
        brush.geometry.max_diameter_px = diameter_px;
        brush.recipe = vec![(slot, 1.0)];
        brush.load = load;
        match tool {
            Tool::Knife => {
                let mixture = self.session.brush.reservoir_mixture(PIGMENTS.len());
                self.session
                    .dip(self.gpu, &mixture, load)
                    .expect("the GPU stays available while painting");
            }
            Tool::Blender => self.session.clean_brush(self.gpu),
            _ => self.session.reload_brush(self.gpu),
        }
    }
}

fn spec() -> CanvasSpec {
    CanvasSpec {
        width: SHEET_WIDTH_PX,
        height: SHEET_HEIGHT_PX,
        primer: Primer::WhiteGesso,
        weave: Weave::Linen,
        pigment_slots: PIGMENTS.iter().map(|id| id.to_string()).collect(),
    }
}

fn paint_stripes(painter: &mut Painter<'_>) {
    let top = f64::from(SHEET_HEIGHT_PX) * 0.12;
    let bottom = f64::from(SHEET_HEIGHT_PX) * 0.88;
    for x in STRIPE_XS_PX {
        painter.take_up(Tool::Brush, STRIPE_DIAMETER_PX, 1.0, YELLOW);
        let path = |along: f64| (x, top + (bottom - top) * along);
        painter.stroke(&walk(path, STRIPE_SPEED_PX_PER_SECOND, |_| 0.9));
    }
}

/// Pen samples along `path` (a point for each share of the way, 0 to 1), taken at
/// `SAMPLES_PER_SECOND` while the pen moves at `speed_px_per_second`.
fn walk(
    path: impl Fn(f64) -> (f64, f64),
    speed_px_per_second: f64,
    pressure: impl Fn(f64) -> f32,
) -> Vec<StrokeSample> {
    let spacing_px = speed_px_per_second / SAMPLES_PER_SECOND;
    let mut samples = Vec::new();
    let mut walked_px = 0.0;
    let mut previous = path(0.0);
    for step in 0..=PATH_STEPS {
        let along = step as f64 / PATH_STEPS as f64;
        let point = path(along);
        walked_px += (point.0 - previous.0).hypot(point.1 - previous.1);
        previous = point;
        if walked_px >= samples.len() as f64 * spacing_px || step == PATH_STEPS {
            samples.push(StrokeSample {
                x: point.0,
                y: point.1,
                pressure: pressure(along),
                tilt_x_deg: STROKE_TILT_DEG,
                tilt_y_deg: 0.0,
                time_ms: (samples.len() as f64 * 1000.0 / SAMPLES_PER_SECOND) as u32,
            });
        }
    }
    samples
}

fn test_pressure(along: f64) -> f32 {
    let along = along as f32;
    if along < 0.5 {
        PRESSURE_START + (PRESSURE_PEAK - PRESSURE_START) * along * 2.0
    } else {
        PRESSURE_PEAK + (PRESSURE_END - PRESSURE_PEAK) * (along - 0.5) * 2.0
    }
}

fn test_stroke(speed_px_per_second: f64) -> Vec<StrokeSample> {
    let width = f64::from(SHEET_WIDTH_PX);
    let middle = f64::from(SHEET_HEIGHT_PX) / 2.0;
    let path = |along: f64| {
        (
            STROKE_MARGIN_PX + (width - 2.0 * STROKE_MARGIN_PX) * along,
            middle + STROKE_AMPLITUDE_PX * (TAU * along).sin(),
        )
    };
    walk(path, speed_px_per_second, test_pressure)
}

fn apply(candidate: Candidate, session: &mut PaintSession) {
    match candidate {
        Candidate::Reference => {}
        Candidate::FrameLimit(limit) => session.limit_dabs_per_frame(Some(limit)),
        Candidate::Spacing(share) => session.brush.geometry.spacing = share,
    }
}

pub(crate) fn paint_cell(gpu: &GpuContext, cell: &Cell, candidate: Candidate) -> Vec<u8> {
    presented(gpu, &painted_session(gpu, cell, candidate))
}

pub(crate) fn painted_session(gpu: &GpuContext, cell: &Cell, candidate: Candidate) -> PaintSession {
    let session = PaintSession::new(gpu, spec()).expect("the sheet canvas is valid");
    let mut painter = Painter {
        gpu,
        session,
        frame_index: 0,
    };
    paint_stripes(&mut painter);
    painter.take_up(cell.tool, cell.diameter_px, cell.load, BLUE);
    apply(candidate, &mut painter.session);
    painter.stroke(&test_stroke(cell.speed_px_per_second));
    painter.session
}

fn presented(gpu: &GpuContext, session: &PaintSession) -> Vec<u8> {
    let target = session
        .presenter
        .create_target(gpu, SHEET_WIDTH_PX, SHEET_HEIGHT_PX);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let view = ViewTransform {
        origin_x: 0.0,
        origin_y: 0.0,
        zoom: 1.0,
        rotation: 0.0,
    };
    session.presenter.encode_present(
        gpu,
        PresentRequest::of_canvas(&session.canvas, view),
        &target,
        &mut encoder,
    );
    gpu.queue.submit([encoder.finish()]);
    readback_rgba8(gpu, &target).expect("the GPU stays available while reading the sheet")
}
