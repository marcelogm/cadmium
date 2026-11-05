use cadmium_engine::{
    canvas::{CanvasSpec, Primer, StateTexture, Weave},
    document::{self, ExportedImage},
    gpu::GpuContext,
    render::ViewMode,
    session::PaintSession,
};

use crate::{TempDir, painted_session, painted_spec, squeeze_on_the_tray};

#[test]
fn export_png_has_canvas_dimensions() {
    let gpu = GpuContext::new().unwrap();
    let mut s = PaintSession::new(
        &gpu,
        CanvasSpec {
            width: 300,
            height: 200,
            primer: Primer::WhiteGesso,
            weave: Weave::Smooth,
            pigment_slots: vec!["titanium_white".into()],
        },
    )
    .unwrap();
    let dir = TempDir::new("export");
    let path = dir.file("export.png");
    document::export_png(&gpu, &mut s, &path).unwrap();
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    assert_eq!((reader.info().width, reader.info().height), (300, 200));
    assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
    let mut pixels = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut pixels).unwrap();
    assert!(pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
}

#[test]
fn export_png_restores_the_view_mode_and_reshades_it() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session(&gpu);
    s.view_mode = ViewMode::Height;
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.canvas.dirty.mark_all();
    s.encode_frame(&gpu, crate::support::FROZEN_FRAME_CLOCK_MS, &mut e);
    gpu.queue.submit([e.finish()]);
    let height_display = s.canvas.read_bytes(&gpu, StateTexture::Display).unwrap();
    let dir = TempDir::new("export-mode");
    let path = dir.file("export-mode.png");
    document::export_png(&gpu, &mut s, &path).unwrap();
    assert_eq!(s.view_mode, ViewMode::Height);
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(&gpu, crate::support::FROZEN_FRAME_CLOCK_MS, &mut e);
    gpu.queue.submit([e.finish()]);
    assert_eq!(
        s.canvas.read_bytes(&gpu, StateTexture::Display).unwrap(),
        height_display
    );
}

#[test]
fn export_ignores_the_tray() {
    let gpu = GpuContext::new().unwrap();
    let mut fresh = PaintSession::new(&gpu, painted_spec()).unwrap();
    let mut with_tray_paint = PaintSession::new(&gpu, painted_spec()).unwrap();
    squeeze_on_the_tray(&gpu, &mut with_tray_paint);
    assert!(with_tray_paint.tray.wet_totals(&gpu).unwrap().volume > 0.0);
    let fresh = ExportedImage::render(&gpu, &mut fresh).unwrap();
    let with_tray_paint = ExportedImage::render(&gpu, &mut with_tray_paint).unwrap();
    assert_eq!(
        (with_tray_paint.width, with_tray_paint.height),
        (fresh.width, fresh.height)
    );
    assert!(with_tray_paint.rgba == fresh.rgba);
}
