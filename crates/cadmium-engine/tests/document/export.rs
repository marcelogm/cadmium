use cadmium_engine::{
    canvas::{CanvasSpec, MARGIN_PX, Primer, StateTexture, Weave},
    document::{self, ExportedImage},
    gpu::GpuContext,
    render::ViewMode,
    session::PaintSession,
};

use crate::support::{load, painting_session, stroke};
use crate::{TempDir, painted_session};

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
            margin_px: 0,
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
fn the_exported_picture_is_the_painting_without_its_margin() {
    let gpu = GpuContext::new().unwrap();
    let mut clean = painting_session(&gpu, 300, 200);
    let mut dirty = painting_session(&gpu, 300, 200);
    load(&gpu, &mut dirty, 0);
    stroke(&gpu, &mut dirty, f64::from(MARGIN_PX) / 2.0, 40.0, 400.0);
    let clean = ExportedImage::render(&gpu, &mut clean).unwrap();
    let dirty = ExportedImage::render(&gpu, &mut dirty).unwrap();
    assert_eq!((dirty.width, dirty.height), (300, 200));
    assert!(dirty.rgba == clean.rgba);
}

#[test]
fn paint_on_the_painting_is_exported_where_it_was_laid() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 300, 200);
    let margin = f64::from(MARGIN_PX);
    load(&gpu, &mut s, 0);
    stroke(&gpu, &mut s, margin + 100.0, margin + 20.0, margin + 280.0);
    let image = ExportedImage::render(&gpu, &mut s).unwrap();
    let pixel = |x: u32, y: u32| &image.rgba[((y * image.width + x) * 4) as usize..][..3];
    assert_ne!(pixel(150, 100), pixel(150, 10));
}
