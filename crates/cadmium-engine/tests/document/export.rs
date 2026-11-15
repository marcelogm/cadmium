use cadmium_engine::{
    canvas::{CanvasSpec, MARGIN_PX, Primer, StateTexture, Weave},
    document::{self, ExportedImage},
    gpu::GpuContext,
    render::ViewMode,
    session::PaintSession,
};

use crate::support::{load, painting_session, presented_rgba, stroke};
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

const RGB: usize = 3;
const RELIEF_REACH: u32 = 1;

fn rgb_at(rgba: &[u8], width: u32, x: u32, y: u32) -> &[u8] {
    &rgba[((y * width + x) * 4) as usize..][..RGB]
}

fn within_one(a: &[u8], b: &[u8]) -> bool {
    a.iter().zip(b).all(|(a, b)| a.abs_diff(*b) <= 1)
}

#[test]
fn the_exported_picture_is_the_painting_without_its_margin() {
    let gpu = GpuContext::new().unwrap();
    let mut clean = painting_session(&gpu, 300, 200);
    let mut dirty = painting_session(&gpu, 300, 200);
    load(&gpu, &mut dirty, 0);
    stroke(&gpu, &mut dirty, f64::from(MARGIN_PX) / 2.0, 40.0, 400.0);
    let exported_clean = ExportedImage::render(&gpu, &mut clean).unwrap();
    let dirty = ExportedImage::render(&gpu, &mut dirty).unwrap();
    assert_eq!((dirty.width, dirty.height), (300, 200));
    assert!(dirty.rgba == exported_clean.rgba);
    let margin = presented_rgba(&gpu, &clean, 1, 1);
    let (w, h) = (exported_clean.width, exported_clean.height);
    let edges = (0..w)
        .flat_map(|x| [(x, 0), (x, h - 1)])
        .chain((0..h).flat_map(|y| [(0, y), (w - 1, y)]));
    for (x, y) in edges {
        let pixel = rgb_at(&exported_clean.rgba, w, x, y);
        assert!(
            !within_one(pixel, &margin[..RGB]),
            "margin colour at ({x}, {y})"
        );
    }
}

/// Checks that every exported pixel shows the texel under it on the whole canvas.
fn assert_shows_the_canvas_under_the_painting(
    gpu: &GpuContext,
    s: &PaintSession,
    image: &ExportedImage,
) {
    let surface = s.canvas.spec.width;
    let presented = presented_rgba(gpu, s, surface, s.canvas.spec.height);
    for y in 0..image.height {
        for x in 0..image.width {
            let exported = rgb_at(&image.rgba, image.width, x, y);
            let on_the_canvas = rgb_at(&presented, surface, x + MARGIN_PX, y + MARGIN_PX);
            assert!(
                within_one(exported, on_the_canvas),
                "pixel ({x}, {y}): {exported:?} against {on_the_canvas:?}"
            );
        }
    }
}

fn rows_that_differ(a: &ExportedImage, b: &ExportedImage) -> Vec<u32> {
    (0..a.height)
        .filter(|&y| {
            (0..a.width).any(|x| rgb_at(&a.rgba, a.width, x, y) != rgb_at(&b.rgba, b.width, x, y))
        })
        .collect()
}

/// The painting's rows, counted from its first texel, that hold wet paint.
fn wet_rows_of_the_painting(gpu: &GpuContext, s: &PaintSession) -> Vec<u32> {
    let rect = s.canvas.spec.painting_rect();
    let wet = s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap();
    (0..rect.height)
        .filter(|&y| {
            let row = ((y + rect.y) * s.canvas.spec.width + rect.x) as usize;
            wet[row..][..rect.width as usize]
                .iter()
                .any(|volume| *volume > 0.0)
        })
        .collect()
}

#[test]
fn paint_on_the_painting_is_exported_where_it_was_laid() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 300, 200);
    let mut clean = painting_session(&gpu, 300, 200);
    let margin = f64::from(MARGIN_PX);
    load(&gpu, &mut s, 0);
    stroke(&gpu, &mut s, margin + 100.0, margin + 20.0, margin + 280.0);
    let image = ExportedImage::render(&gpu, &mut s).unwrap();
    let clean = ExportedImage::render(&gpu, &mut clean).unwrap();
    assert_ne!(
        rgb_at(&image.rgba, image.width, 150, 100),
        rgb_at(&image.rgba, image.width, 150, 10)
    );
    assert_shows_the_canvas_under_the_painting(&gpu, &s, &image);
    let wet_rows = wet_rows_of_the_painting(&gpu, &s);
    let (first, last) = (wet_rows[0], wet_rows[wet_rows.len() - 1]);
    // The relief's shading reaches one row past the paint on either side.
    let band: Vec<u32> = (first - RELIEF_REACH..=last + RELIEF_REACH).collect();
    assert_eq!(
        rows_that_differ(&image, &clean),
        band,
        "wet rows {wet_rows:?}"
    );
}
