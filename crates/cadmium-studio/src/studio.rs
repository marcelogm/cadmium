//! The real simulation on a small canvas, isolated from the painting.

use std::fmt;

use cadmium_engine::brush::BrushSpec;
use cadmium_engine::canvas::{Canvas, CanvasError, CanvasSpec, Primer, Weave};
use cadmium_engine::gpu::{GpuContext, GpuLost};
use cadmium_engine::present::{PresentRequest, ViewTransform, readback_rgba8};
use cadmium_engine::session::PaintSession;

use crate::demo::{DemoStroke, StrokeBox};
use crate::loading::{Loading, wear};
use crate::paint::DEMO_PIGMENT;
use crate::schedule::{StrokeSchedule, replay};

/// A rack row's stroke (10:3), shown 153x46 px. The rack takes `thumbnail_full`, at
/// `STUDIO_SCALE` times this, so a row stays sharp at a screen scale of 2.
pub const THUMBNAIL_SIZE: (u32, u32) = (240, 72);
const STUDIO_SCALE: u32 = 2;
/// Small enough that a fan's splayed hairs stay inside the thumbnail.
const STUDIO_MAX_SIZE_PX: f32 = 44.0;
const STUDIO_MEDIUM: f32 = 0.1;
const STUDIO_LOAD: f32 = 1.0;
/// 15% of the canvas width (72 px at 480): room for a fan's splay at the 44 px cap.
const MARGIN_X_SHARE: f64 = 0.15;
/// Wider than the side margin: the stroke's width spreads up and down.
const MARGIN_Y_SHARE: f64 = 0.30;

#[derive(Clone, Debug, PartialEq, Eq)]
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
///
/// Every run starts its clock at 0 on a fresh canvas with a freshly loaded brush,
/// so the same brush and stroke always paint the same pixels.
pub struct BrushStudio {
    session: PaintSession,
}

fn studio_spec() -> CanvasSpec {
    CanvasSpec {
        width: THUMBNAIL_SIZE.0 * STUDIO_SCALE,
        height: THUMBNAIL_SIZE.1 * STUDIO_SCALE,
        primer: Primer::WhiteGesso,
        weave: Weave::Smooth,
        pigment_slots: vec![DEMO_PIGMENT.to_string()],
    }
}

impl BrushStudio {
    pub fn new(gpu: &GpuContext) -> Result<BrushStudio, StudioError> {
        Ok(BrushStudio {
            session: PaintSession::headless(gpu, studio_spec())?,
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
        let schedule = StrokeSchedule::thumbnail(stroke, self.stroke_box());
        replay(&mut self.session, gpu, schedule.events());
        Ok(())
    }

    /// The brush's S-curve, at thumbnail size.
    pub fn thumbnail(
        &mut self,
        gpu: &GpuContext,
        brush: &BrushSpec,
    ) -> Result<Thumbnail, StudioError> {
        self.run(gpu, brush, DemoStroke::SCurve)?;
        Ok(self.capture(gpu, 1)?)
    }

    /// The brush's S-curve at the studio canvas's own resolution, twice `THUMBNAIL_SIZE`:
    /// sharp where a thumbnail pixel spans more than one screen pixel.
    pub fn thumbnail_full(
        &mut self,
        gpu: &GpuContext,
        brush: &BrushSpec,
    ) -> Result<Thumbnail, StudioError> {
        self.run(gpu, brush, DemoStroke::SCurve)?;
        Ok(self.capture(gpu, STUDIO_SCALE)?)
    }

    /// The canvas as shown, at thumbnail size, as straight sRGB RGBA8.
    pub fn rgba(&self, gpu: &GpuContext) -> Result<Vec<u8>, GpuLost> {
        Ok(self.capture(gpu, 1)?.rgba)
    }

    /// The canvas as shown at `scale` times `THUMBNAIL_SIZE`, `scale` at most `STUDIO_SCALE`.
    fn capture(&self, gpu: &GpuContext, scale: u32) -> Result<Thumbnail, GpuLost> {
        debug_assert!((1..=STUDIO_SCALE).contains(&scale));
        let (width, height) = (THUMBNAIL_SIZE.0 * scale, THUMBNAIL_SIZE.1 * scale);
        let presenter = &self.session.presenter;
        let target = presenter.create_target(gpu, width, height);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let request = PresentRequest::of_canvas(
            &self.session.canvas,
            ViewTransform {
                origin_x: 0.0,
                origin_y: 0.0,
                zoom: scale as f32 / STUDIO_SCALE as f32,
                rotation: 0.0,
            },
        );
        presenter.encode_present(gpu, request, &target, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        let rgba = readback_rgba8(gpu, &target)?;
        Ok(Thumbnail {
            width,
            height,
            rgba,
        })
    }

    /// Holds `brush` at a size that fits, freshly loaded with the studio paint.
    fn wear(&mut self, gpu: &GpuContext, brush: &BrushSpec) {
        let mut fitted = brush.clone();
        fitted.size_px = fitted.size_px.min(STUDIO_MAX_SIZE_PX);
        let loading = Loading {
            recipe: &[(0, 1.0)],
            load: STUDIO_LOAD,
            medium: STUDIO_MEDIUM,
        };
        wear(&mut self.session, gpu, &fitted, loading);
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
}
