use std::f64::consts::TAU;
use std::time::Instant;

use cadmium_engine::gpu::GpuContext;
use cadmium_engine::present::{PresentRequest, ViewTransform};
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::{StrokeSample, Tool};

use crate::scenarios::Scenario;
use crate::timing::{FrameCost, Timestamps, finish_frame, write_timestamp};
use crate::{CANVAS_HEIGHT_PX, CANVAS_WIDTH_PX, PIGMENTS};

const CURVE_LENGTH_PX: f64 = 2000.0;
const CURVE_SPAN_PX: f64 = 1500.0;
const CURVE_AMPLITUDE_PX: f64 = 320.0;
const CURVE_WALK_STEPS: usize = 20_000;
const PEN_SPEED_PX_PER_SECOND: f64 = 800.0;
const SAMPLES_PER_SECOND: f64 = 120.0;
const FRAMES_PER_SECOND: f64 = 60.0;
pub(crate) const STROKE_PRESSURE: f32 = 0.8;
const STROKE_TILT_DEG: f32 = 20.0;
const IMPASTO_DIAMETER_PX: f32 = 300.0;
const IMPASTO_PASSES: usize = 4;
const FULL_LOAD: f32 = 1.0;
const FULL_DIP: f32 = 1.0;
const REST_BEFORE_MEASURING_MS: f64 = 5000.0;
const STILL_FRAMES: usize = 120;
const LONG_STROKE_PASSES: usize = 4;

pub(crate) struct Bench<'gpu> {
    pub(crate) gpu: &'gpu GpuContext,
    pub(crate) session: PaintSession,
    pub(crate) timestamps: Option<&'gpu Timestamps>,
    pub(crate) present_target: Option<wgpu::Texture>,
    pub(crate) frame_index: u32,
}

impl Bench<'_> {
    fn frame_clock_ms(&self) -> f64 {
        f64::from(self.frame_index) * 1000.0 / FRAMES_PER_SECOND
    }

    fn frame(&mut self) -> FrameCost {
        let started = Instant::now();
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        if let Some(timestamps) = self.timestamps {
            write_timestamp(&mut encoder, timestamps, Some(0), None);
        }
        self.session
            .encode_frame(self.gpu, self.frame_clock_ms(), &mut encoder);
        if let Some(target) = &self.present_target {
            encode_full_present(self.gpu, &self.session, target, &mut encoder);
        }
        let gpu_ms = finish_frame(self.gpu, self.timestamps, encoder);
        self.frame_index += 1;
        FrameCost {
            gpu_ms,
            wall_ms: started.elapsed().as_secs_f64() * 1000.0,
        }
    }

    pub(crate) fn stroke(&mut self, samples: &[StrokeSample]) -> Vec<FrameCost> {
        let Some((&first, rest)) = samples.split_first() else {
            return Vec::new();
        };
        let samples_per_frame = (SAMPLES_PER_SECOND / FRAMES_PER_SECOND) as usize;
        self.session.begin_stroke(self.gpu, first);
        let mut costs = vec![self.frame()];
        for chunk in rest.chunks(samples_per_frame) {
            self.session.add_samples(chunk);
            costs.push(self.frame());
        }
        self.session.end_stroke();
        costs.push(self.frame());
        costs
    }

    fn rest(&mut self) {
        self.frame_index += (REST_BEFORE_MEASURING_MS * FRAMES_PER_SECOND / 1000.0) as u32;
        self.frame();
    }

    fn hold(&mut self, tool: Tool, diameter_px: f32, load: f32) {
        self.session.brush.tool = tool;
        self.session.brush.geometry.max_diameter_px = diameter_px;
        self.session.brush.load = load;
        self.session.reload_brush(self.gpu);
    }

    pub(crate) fn lay_impasto(&mut self) {
        let curve = s_curve(1.0);
        for _ in 0..IMPASTO_PASSES {
            self.hold(Tool::Brush, IMPASTO_DIAMETER_PX, FULL_LOAD);
            self.stroke(&curve);
        }
        self.rest();
    }

    pub(crate) fn take_up(&mut self, scenario: &Scenario) {
        if scenario.tool == Tool::Knife {
            self.session.brush.tool = Tool::Knife;
            self.session.brush.geometry.max_diameter_px = scenario.diameter_px;
            let mixture = self.session.brush.reservoir_mixture(PIGMENTS.len());
            self.session
                .dip(self.gpu, &mixture, FULL_DIP)
                .expect("the GPU stays available while measuring");
            return;
        }
        self.hold(scenario.tool, scenario.diameter_px, FULL_LOAD);
        if scenario.tool == Tool::Blender {
            self.session.clean_brush(self.gpu);
        }
    }

    pub(crate) fn settle(&mut self) -> Vec<FrameCost> {
        self.stroke(&s_curve(STROKE_PRESSURE));
        let mut costs = Vec::new();
        while self.session.bristles_settling(self.frame_clock_ms()) {
            costs.push(self.frame());
        }
        costs
    }

    pub(crate) fn shade(&mut self) -> Vec<FrameCost> {
        (0..STILL_FRAMES)
            .map(|_| {
                self.session.canvas.dirty.mark_all();
                self.frame()
            })
            .collect()
    }

    pub(crate) fn present(&mut self) -> Vec<FrameCost> {
        self.present_target = Some(self.session.presenter.create_target(
            self.gpu,
            CANVAS_WIDTH_PX,
            CANVAS_HEIGHT_PX,
        ));
        (0..STILL_FRAMES).map(|_| self.frame()).collect()
    }
}

fn encode_full_present(
    gpu: &GpuContext,
    session: &PaintSession,
    target: &wgpu::Texture,
    encoder: &mut wgpu::CommandEncoder,
) {
    let request = PresentRequest {
        canvas: &session.canvas,
        view: ViewTransform {
            origin_x: 0.0,
            origin_y: 0.0,
            zoom: 1.0,
        },
    };
    session
        .presenter
        .encode_present(gpu, request, target, encoder);
}

fn s_curve_point(along: f64) -> (f64, f64) {
    let centre_x = f64::from(CANVAS_WIDTH_PX) / 2.0;
    let centre_y = f64::from(CANVAS_HEIGHT_PX) / 2.0;
    (
        centre_x + CURVE_SPAN_PX * (along - 0.5),
        centre_y + CURVE_AMPLITUDE_PX * (TAU * along).sin(),
    )
}

pub(crate) fn long_stroke(pressure: f32) -> Vec<StrokeSample> {
    let forward = s_curve(pressure);
    let mut samples = Vec::new();
    for pass in 0..LONG_STROKE_PASSES {
        let mut leg = forward.clone();
        if pass % 2 == 1 {
            leg.reverse();
        }
        samples.extend(leg);
    }
    for (index, sample) in samples.iter_mut().enumerate() {
        sample.time_ms = (index as f64 * 1000.0 / SAMPLES_PER_SECOND) as u32;
    }
    samples
}

pub(crate) fn s_curve(pressure: f32) -> Vec<StrokeSample> {
    let spacing_px = PEN_SPEED_PX_PER_SECOND / SAMPLES_PER_SECOND;
    let mut samples = Vec::new();
    let mut walked_px = 0.0;
    let mut previous = s_curve_point(0.0);
    for step in 0..=CURVE_WALK_STEPS {
        let point = s_curve_point(step as f64 / CURVE_WALK_STEPS as f64);
        walked_px += (point.0 - previous.0).hypot(point.1 - previous.1);
        previous = point;
        let due = samples.len() as f64 * spacing_px;
        if walked_px >= due && due <= CURVE_LENGTH_PX {
            samples.push(sample_at(point, samples.len(), pressure));
        }
    }
    samples
}

fn sample_at((x, y): (f64, f64), index: usize, pressure: f32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure,
        tilt_x_deg: STROKE_TILT_DEG,
        tilt_y_deg: 0.0,
        time_ms: (index as f64 * 1000.0 / SAMPLES_PER_SECOND) as u32,
    }
}
