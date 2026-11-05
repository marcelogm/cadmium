//! Drives a paint session the way a hand at the easel would, frame by frame.

use cadmium_engine::brush::BrushShape;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;

use crate::hand::{Gesture, Point, Rng, Touch, clip, vary};

const FRAME_MS: f64 = 16.0;
const SAMPLES_PER_FRAME: usize = 4;
const MAX_SETTLE_FRAMES: usize = 90;
const MIX_JITTER: f64 = 0.08;

/// Which brush, how big, and how it was loaded.
#[derive(Clone, Copy)]
pub struct Brush {
    pub shape: BrushShape,
    pub diameter: f32,
    pub load: f32,
    pub medium: f32,
}

pub struct Painter {
    pub gpu: GpuContext,
    pub session: PaintSession,
    pub rng: Rng,
    clock_ms: f64,
}

impl Painter {
    pub fn new(gpu: GpuContext, session: PaintSession, seed: u64) -> Painter {
        Painter {
            gpu,
            session,
            rng: Rng::new(seed),
            clock_ms: 0.0,
        }
    }

    pub fn load(&mut self, brush: Brush, recipe: &[(usize, f32)]) {
        let settings = &mut self.session.brush;
        settings.shape = brush.shape;
        settings.recipe = vary(&mut self.rng, recipe, MIX_JITTER);
        settings.geometry.max_diameter_px = brush.diameter;
        settings.load = brush.load;
        settings.medium = brush.medium;
        self.session.reload_brush(&self.gpu);
    }

    fn frame(&mut self) {
        self.clock_ms += FRAME_MS;
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        self.session
            .encode_frame(&self.gpu, self.clock_ms, &mut encoder);
        self.gpu.queue.submit([encoder.finish()]);
    }

    /// One stroke, then a pause off the canvas while the hairs spring back.
    pub fn stroke(&mut self, path: &[Point]) {
        let touch = Touch::random(&mut self.rng);
        self.stroke_with(&touch, path);
    }

    /// A stroke with the handle held upright, for dabs that turn on the spot.
    pub fn stroke_upright(&mut self, path: &[Point]) {
        let touch = Touch::random(&mut self.rng).upright();
        self.stroke_with(&touch, path);
    }

    fn stroke_with(&mut self, touch: &Touch, path: &[Point]) {
        let samples = touch.samples(path);
        let Some((&first, rest)) = samples.split_first() else {
            return;
        };
        self.session.begin_stroke(&self.gpu, first);
        self.frame();
        for chunk in rest.chunks(SAMPLES_PER_FRAME) {
            self.session.add_samples(chunk);
            self.frame();
        }
        self.session.end_stroke();
        self.frame();
        let mut settled = 0;
        while settled < MAX_SETTLE_FRAMES && self.session.bristles_settling(self.clock_ms) {
            self.frame();
            settled += 1;
        }
    }

    /// The stroke `gesture` describes, cut to the stretch that stays where `inside` allows.
    pub fn gesture(&mut self, gesture: &Gesture, inside: impl Fn(Point) -> bool) {
        let path = gesture.path(&mut self.rng);
        let kept = clip(&path, inside);
        if kept.len() > 3 {
            self.stroke(kept);
        }
    }

    pub fn dry(&mut self) {
        self.session.dry(&self.gpu);
        self.frame();
    }
}
