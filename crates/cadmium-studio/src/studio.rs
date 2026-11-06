//! The real simulation on a small canvas, isolated from the painting.

use std::fmt;

use cadmium_engine::brush::BrushSpec;
use cadmium_engine::canvas::{Canvas, CanvasError, CanvasSpec, Primer, Weave};
use cadmium_engine::gpu::{GpuContext, GpuLost};
use cadmium_engine::present::{PresentRequest, ViewTransform, readback_rgba8};
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::demo::{DemoStroke, StrokeBox, samples};

pub const THUMBNAIL_SIZE: (u32, u32) = (192, 64);
const STUDIO_SCALE: u32 = 2;
/// Small enough that a fan's splayed hairs stay inside the thumbnail.
const STUDIO_MAX_SIZE_PX: f32 = 44.0;
const STUDIO_PIGMENT: &str = "burnt_umber";
const STUDIO_MEDIUM: f32 = 0.1;
const STUDIO_LOAD: f32 = 1.0;
const FRAME_EVERY_SAMPLES: usize = 3;
const SETTLE_FRAMES: u32 = 20;
const SETTLE_STEP_MS: f64 = 60.0;
const MARGIN_X_SHARE: f64 = 0.18;
/// Wider than the side margin: the stroke's width spreads up and down.
const MARGIN_Y_SHARE: f64 = 0.33;

pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    /// Straight sRGB-encoded RGBA8, row-major, no padding, opaque.
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StudioError {
    Canvas(CanvasError),
    Gpu(GpuLost),
}

impl From<CanvasError> for StudioError {
    fn from(error: CanvasError) -> StudioError {
        StudioError::Canvas(error)
    }
}

impl From<GpuLost> for StudioError {
    fn from(error: GpuLost) -> StudioError {
        StudioError::Gpu(error)
    }
}

impl fmt::Display for StudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StudioError::Canvas(error) => write!(f, "brush studio canvas: {error}"),
            StudioError::Gpu(error) => write!(f, "brush studio: {error}"),
        }
    }
}

impl std::error::Error for StudioError {}

/// A private paint session that shows what a brush does.
pub struct BrushStudio {
    session: PaintSession,
    clock_ms: f64,
}

fn studio_spec() -> CanvasSpec {
    CanvasSpec {
        width: THUMBNAIL_SIZE.0 * STUDIO_SCALE,
        height: THUMBNAIL_SIZE.1 * STUDIO_SCALE,
        primer: Primer::WhiteGesso,
        weave: Weave::Smooth,
        pigment_slots: vec![STUDIO_PIGMENT.to_string()],
    }
}

impl BrushStudio {
    pub fn new(gpu: &GpuContext) -> Result<BrushStudio, CanvasError> {
        Ok(BrushStudio {
            session: PaintSession::new(gpu, studio_spec())?,
            clock_ms: 0.0,
        })
    }

    /// Paints `stroke` with `brush` on a fresh canvas.
    pub fn run(
        &mut self,
        gpu: &GpuContext,
        brush: &BrushSpec,
        stroke: DemoStroke,
    ) -> Result<(), StudioError> {
        self.session.canvas = Canvas::new(gpu, &self.session.db, studio_spec())?;
        self.wear(gpu, brush);
        self.paint(gpu, stroke);
        Ok(())
    }

    /// The brush's S-curve, at thumbnail size.
    pub fn thumbnail(
        &mut self,
        gpu: &GpuContext,
        brush: &BrushSpec,
    ) -> Result<Thumbnail, StudioError> {
        self.run(gpu, brush, DemoStroke::SCurve)?;
        let rgba = self.rgba(gpu)?;
        Ok(Thumbnail {
            width: THUMBNAIL_SIZE.0,
            height: THUMBNAIL_SIZE.1,
            rgba,
        })
    }

    /// The canvas as shown, at thumbnail size, as straight sRGB RGBA8.
    pub fn rgba(&self, gpu: &GpuContext) -> Result<Vec<u8>, GpuLost> {
        let presenter = &self.session.presenter;
        let target = presenter.create_target(gpu, THUMBNAIL_SIZE.0, THUMBNAIL_SIZE.1);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let request = PresentRequest {
            canvas: &self.session.canvas,
            view: ViewTransform {
                origin_x: 0.0,
                origin_y: 0.0,
                zoom: 1.0 / STUDIO_SCALE as f32,
            },
        };
        presenter.encode_present(gpu, request, &target, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        readback_rgba8(gpu, &target)
    }

    /// Holds `brush` at a size that fits, freshly loaded with the studio paint.
    fn wear(&mut self, gpu: &GpuContext, brush: &BrushSpec) {
        let mut fitted = brush.clone();
        fitted.size_px = fitted.size_px.min(STUDIO_MAX_SIZE_PX);
        let settings = &mut self.session.brush;
        settings.wear(&fitted);
        settings.recipe = vec![(0, 1.0)];
        settings.load = STUDIO_LOAD;
        settings.medium = STUDIO_MEDIUM;
        self.session.reload_brush(gpu);
    }

    fn paint(&mut self, gpu: &GpuContext, stroke: DemoStroke) {
        let start_ms = self.clock_ms.ceil() as u32;
        for (index, sample) in samples(stroke, self.stroke_box()).iter().enumerate() {
            let sample = StrokeSample {
                time_ms: start_ms + sample.time_ms,
                ..*sample
            };
            self.touch(gpu, index, sample);
        }
        self.session.end_stroke();
        for _ in 0..SETTLE_FRAMES {
            self.clock_ms += SETTLE_STEP_MS;
            self.frame(gpu);
        }
    }

    /// Feeds one sample of the stroke, showing a frame every few samples.
    fn touch(&mut self, gpu: &GpuContext, index: usize, sample: StrokeSample) {
        if index == 0 {
            self.session.begin_stroke(gpu, sample);
        } else {
            self.session.add_samples(&[sample]);
        }
        self.clock_ms = f64::from(sample.time_ms);
        if index.is_multiple_of(FRAME_EVERY_SAMPLES) {
            self.frame(gpu);
        }
    }

    fn stroke_box(&self) -> StrokeBox {
        let spec = &self.session.canvas.spec;
        let (width, height) = (f64::from(spec.width), f64::from(spec.height));
        StrokeBox {
            x: width * MARGIN_X_SHARE,
            y: height * MARGIN_Y_SHARE,
            width: width * (1.0 - 2.0 * MARGIN_X_SHARE),
            height: height * (1.0 - 2.0 * MARGIN_Y_SHARE),
        }
    }

    fn frame(&mut self, gpu: &GpuContext) {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        self.session.encode_frame(gpu, self.clock_ms, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
    }
}
