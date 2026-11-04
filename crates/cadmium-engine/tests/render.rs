pub mod support;

use cadmium_engine::{
    canvas::*, gpu::GpuContext, params::SimParams, pigments::PigmentDb, present::*, render::*,
};
use support::write_png;

fn blank(gpu: &GpuContext) -> Canvas {
    Canvas::new(
        gpu,
        &PigmentDb::builtin(),
        CanvasSpec {
            width: 128,
            height: 128,
            primer: Primer::WhiteGesso,
            weave: Weave::Smooth,
            pigment_slots: vec!["ultramarine_blue".into()],
        },
    )
    .unwrap()
}

fn shade_and_present(
    gpu: &GpuContext,
    canvas: &mut Canvas,
    view: ViewTransform,
    w: u32,
    h: u32,
) -> Vec<u8> {
    let renderer = Renderer::new(gpu);
    let presenter = Presenter::new(gpu);
    let target = presenter.create_target(gpu, w, h);
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    renderer.encode_shade(
        gpu,
        canvas,
        &SimParams::default(),
        ViewMode::Color,
        &mut enc,
    );
    presenter.encode_present(gpu, canvas, view, &target, &mut enc);
    gpu.queue.submit([enc.finish()]);
    readback_rgba8(gpu, &target).unwrap()
}

const IDENTITY_VIEW: ViewTransform = ViewTransform {
    origin_x: 0.0,
    origin_y: 0.0,
    zoom: 1.0,
};

#[test]
fn blank_white_gesso_presents_as_titanium_white_masstone() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = blank(&gpu);
    let px = shade_and_present(&gpu, &mut canvas, IDENTITY_VIEW, 128, 128);
    let expected = PigmentDb::builtin().masstone_srgb8("titanium_white");
    let i = (64 * 128 + 64) * 4;
    for c in 0..3 {
        assert!(
            (px[i + c] as i32 - expected[c] as i32).abs() <= 2,
            "{:?} vs {:?}",
            &px[i..i + 3],
            expected
        );
    }
}

#[test]
fn area_outside_the_canvas_is_background() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = blank(&gpu);
    let px = shade_and_present(
        &gpu,
        &mut canvas,
        ViewTransform {
            origin_x: -50.0,
            origin_y: 0.0,
            zoom: 1.0,
        },
        128,
        128,
    );
    assert_eq!(&px[0..3], &[192, 191, 188]);
}

#[test]
fn wet_ultramarine_over_white_looks_blue() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = blank(&gpu);
    let n = 128 * 128;
    let mut pig = vec![0.0f32; n * 4];
    for i in 0..n {
        pig[i * 4] = 1.0;
    }
    canvas.write_f32(&gpu, StateTexture::WetPig0, &pig);
    canvas.write_f32(&gpu, StateTexture::WetVol, &vec![200.0; n]);
    canvas.dirty.mark_all();
    let px = shade_and_present(&gpu, &mut canvas, IDENTITY_VIEW, 128, 128);
    let i = (64 * 128 + 64) * 4;
    assert!(
        px[i + 2] > px[i] + 40 && px[i + 2] > px[i + 1] + 20,
        "{:?}",
        &px[i..i + 3]
    );
}

#[test]
fn shading_consumes_dirty_tiles() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = blank(&gpu);
    let _ = shade_and_present(&gpu, &mut canvas, IDENTITY_VIEW, 16, 16);
    assert!(canvas.dirty.is_empty());
}

#[test]
#[expect(clippy::excessive_nesting)]
fn impasto_edge_facing_the_light_is_brighter_than_the_edge_in_shadow() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = blank(&gpu);
    let side = 128usize;
    let inside = |x: usize, y: usize| (32..96).contains(&x) && (32..96).contains(&y);
    let mut pig = vec![0.0f32; side * side * 4];
    let mut vol = vec![0.0f32; side * side];
    for y in 0..side {
        for x in 0..side {
            if inside(x, y) {
                pig[(y * side + x) * 4] = 1.0;
                vol[y * side + x] = 200.0;
            }
        }
    }
    canvas.write_f32(&gpu, StateTexture::WetPig0, &pig);
    canvas.write_f32(&gpu, StateTexture::WetVol, &vol);
    canvas.dirty.mark_all();
    let px = shade_and_present(&gpu, &mut canvas, IDENTITY_VIEW, 128, 128);
    write_png("render-ultramarine.png", 128, 128, &px);
    let luma = |x: usize, y: usize| {
        let i = (y * side + x) * 4;
        px[i] as u32 + px[i + 1] as u32 + px[i + 2] as u32
    };
    let (lit_left, shaded_right) = (luma(32, 64), luma(95, 64));
    let (lit_top, shaded_bottom) = (luma(64, 32), luma(64, 95));
    assert!(lit_left > shaded_right + 30, "{lit_left} vs {shaded_right}");
    assert!(lit_top > shaded_bottom + 30, "{lit_top} vs {shaded_bottom}");
}
