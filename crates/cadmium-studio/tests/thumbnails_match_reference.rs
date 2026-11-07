use cadmium_engine::brush::{BrushSpec, factory_brushes};
use cadmium_engine::canvas::{CanvasSpec, Primer, Weave};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::present::{PresentRequest, ViewTransform, readback_rgba8};
use cadmium_engine::session::PaintSession;
use cadmium_studio::{BrushStudio, DemoStroke, StrokeBox, THUMBNAIL_SIZE, samples};

fn reference_frame(gpu: &GpuContext, session: &mut PaintSession, clock_ms: f64) {
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    session.encode_frame(gpu, clock_ms, &mut encoder);
    gpu.queue.submit([encoder.finish()]);
}

fn reference_paint(gpu: &GpuContext, session: &mut PaintSession) {
    let (w, h) = (
        f64::from(session.canvas.spec.width),
        f64::from(session.canvas.spec.height),
    );
    const MARGIN_X: f64 = 0.15;
    const MARGIN_Y: f64 = 0.30;
    let stroke_box = StrokeBox {
        x: w * MARGIN_X,
        y: h * MARGIN_Y,
        width: w * (1.0 - 2.0 * MARGIN_X),
        height: h * (1.0 - 2.0 * MARGIN_Y),
    };
    let mut clock_ms = 0.0;
    for (index, sample) in samples(DemoStroke::SCurve, stroke_box)
        .into_iter()
        .enumerate()
    {
        if index == 0 {
            session.begin_stroke(sample);
        } else {
            session.add_samples(&[sample]);
        }
        if index % 3 == 0 {
            reference_frame(gpu, session, f64::from(sample.time_ms));
        }
        clock_ms = f64::from(sample.time_ms);
    }
    session.end_stroke();
    for _ in 0..20 {
        clock_ms += 60.0;
        reference_frame(gpu, session, clock_ms);
    }
}

/// The frozen pre-`StrokeSchedule` driver, kept to prove the refactor leaves
/// thumbnails byte-identical on any device; edit it only to follow a deliberate studio
/// re-fit. The guard covers the stroke driver in `studio.rs` only: the demo samples
/// (`demo.rs`) and the engine are shared with the studio side, so a change there moves
/// both sides together.
fn reference_thumbnail(gpu: &GpuContext, brush: &BrushSpec) -> Vec<u8> {
    let spec = CanvasSpec {
        width: THUMBNAIL_SIZE.0 * 2,
        height: THUMBNAIL_SIZE.1 * 2,
        primer: Primer::WhiteGesso,
        weave: Weave::Smooth,
        pigment_slots: vec!["ultramarine_blue".to_string()],
    };
    let mut session = PaintSession::headless(gpu, spec).unwrap();

    let mut fitted = brush.clone();
    fitted.size_px = fitted.size_px.min(44.0);
    session.brush.wear(&fitted);
    session.brush.recipe = vec![(0, 1.0)];
    session.brush.load = 1.0;
    session.brush.medium = 0.1;
    session.reload_brush(gpu);

    reference_paint(gpu, &mut session);

    let presenter = &session.presenter;
    let target = presenter.create_target(gpu, THUMBNAIL_SIZE.0, THUMBNAIL_SIZE.1);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let request = PresentRequest::of_canvas(
        &session.canvas,
        ViewTransform {
            origin_x: 0.0,
            origin_y: 0.0,
            zoom: 0.5,
            rotation: 0.0,
        },
    );
    presenter.encode_present(gpu, request, &target, &mut encoder);
    gpu.queue.submit([encoder.finish()]);
    readback_rgba8(gpu, &target).unwrap()
}

#[test]
fn the_studio_paints_every_factory_brush_as_the_reference_driver_does() {
    let gpu = GpuContext::new().unwrap();
    let mut studio = BrushStudio::new(&gpu).unwrap();
    for brush in factory_brushes() {
        let thumb = studio.thumbnail(&gpu, &brush).unwrap();
        assert_eq!(
            thumb.rgba,
            reference_thumbnail(&gpu, &brush),
            "{}",
            brush.name
        );
    }
}
