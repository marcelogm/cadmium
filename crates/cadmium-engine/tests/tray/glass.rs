//! The tray's glass palette: a plate over mid grey, lit by a soft studio reflection, on a
//! darker ground. None of it reaches the painting.

use cadmium_engine::{
    canvas::{Canvas, CanvasSpec, Primer, Weave},
    gpu::GpuContext,
    params::SimParams,
    pigments::PigmentDb,
    present::{PresentRequest, Presenter, ViewTransform, readback_rgba8},
    render::{Renderer, ShadeRequest, ViewMode},
};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 128;

/// The relative luminance of an sRGB8 colour.
fn luminance(srgb: [u8; 3]) -> f32 {
    let linear = srgb.map(|channel| {
        let v = f32::from(channel) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
}

/// The CIE L* of an sRGB8 colour.
pub fn lightness(srgb: [u8; 3]) -> f32 {
    let luminance = luminance(srgb);
    if luminance > 0.008856 {
        116.0 * luminance.cbrt() - 16.0
    } else {
        903.3 * luminance
    }
}

fn blank(gpu: &GpuContext, primer: Primer) -> Canvas {
    Canvas::new(
        gpu,
        &PigmentDb::builtin(),
        CanvasSpec {
            width: WIDTH,
            height: HEIGHT,
            primer,
            weave: Weave::Smooth,
            pigment_slots: vec!["ultramarine_blue".into()],
        },
    )
    .unwrap()
}

fn shown(gpu: &GpuContext, canvas: &mut Canvas, view: ViewTransform) -> Vec<u8> {
    let renderer = Renderer::new(gpu);
    let presenter = Presenter::new(gpu);
    let target = presenter.create_target(gpu, WIDTH, HEIGHT);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let params = SimParams::default();
    let request = ShadeRequest {
        params: &params,
        mode: ViewMode::Color,
    };
    renderer.encode_shade(gpu, canvas, request, &mut encoder);
    presenter.encode_present(
        gpu,
        PresentRequest::of_canvas(canvas, view),
        &target,
        &mut encoder,
    );
    gpu.queue.submit([encoder.finish()]);
    readback_rgba8(gpu, &target).unwrap()
}

fn texel(pixels: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * WIDTH + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2]]
}

/// The mean of `measure` over a 9×9 patch, so the present's dither doesn't decide a
/// comparison.
fn patch_mean(pixels: &[u8], x: u32, y: u32, measure: fn([u8; 3]) -> f32) -> f32 {
    let mut sum = 0.0;
    for dy in 0..9 {
        for dx in 0..9 {
            sum += measure(texel(pixels, x + dx - 4, y + dy - 4));
        }
    }
    sum / 81.0
}

fn patch_lightness(pixels: &[u8], x: u32, y: u32) -> f32 {
    patch_mean(pixels, x, y, lightness)
}

const UNMOVED: ViewTransform = ViewTransform {
    origin_x: 0.0,
    origin_y: 0.0,
    zoom: 1.0,
    rotation: 0.0,
};

/// Zoomed out by half, the canvas centred in the target with room all around.
const ZOOMED_OUT: ViewTransform = ViewTransform {
    origin_x: -(WIDTH as f32) / 2.0,
    origin_y: -(HEIGHT as f32) / 2.0,
    zoom: 0.5,
    rotation: 0.0,
};

#[test]
fn an_empty_painting_shows_one_flat_colour_with_no_reflection() {
    let gpu = GpuContext::new().unwrap();
    let pixels = shown(&gpu, &mut blank(&gpu, Primer::WhiteGesso), UNMOVED);
    let centre = texel(&pixels, WIDTH / 2, HEIGHT / 2);
    let points = (0..HEIGHT)
        .step_by(7)
        .flat_map(|y| (0..WIDTH).step_by(7).map(move |x| (x, y)));
    for (x, y) in points {
        let here = texel(&pixels, x, y);
        let close = (0..3).all(|c| here[c].abs_diff(centre[c]) <= 1);
        assert!(close, "{x},{y}: {here:?} vs {centre:?}");
    }
}

#[test]
fn the_empty_tray_catches_a_clear_but_soft_reflection_toward_its_top_left() {
    let gpu = GpuContext::new().unwrap();
    let pixels = shown(&gpu, &mut blank(&gpu, Primer::Tray), UNMOVED);
    let lit = patch_mean(&pixels, 40, 24, luminance);
    let far = patch_mean(&pixels, WIDTH - 40, HEIGHT - 24, luminance);
    let rise = lit / far - 1.0;
    assert!((0.06..=0.12).contains(&rise), "lit {lit} far {far}: {rise}");
    // Smooth: it falls off steadily from the lit corner across the plate.
    let middle = patch_mean(&pixels, WIDTH / 2, HEIGHT / 2, luminance);
    assert!(lit > middle && middle > far, "{lit} {middle} {far}");
}

#[test]
fn the_tray_plate_sits_on_a_darker_ground_with_a_lit_edge() {
    let gpu = GpuContext::new().unwrap();
    let pixels = shown(&gpu, &mut blank(&gpu, Primer::Tray), ZOOMED_OUT);
    // The plate covers x 64..192, y 32..96 of the target.
    let ground = patch_lightness(&pixels, 20, 16);
    let plate = patch_lightness(&pixels, 128, 64);
    assert!(ground + 10.0 < plate, "ground {ground} plate {plate}");
    let edge = lightness(texel(&pixels, 128, 32));
    let inside = lightness(texel(&pixels, 128, 40));
    assert!(edge > inside + 2.0, "edge {edge} inside {inside}");
    let shadow = lightness(texel(&pixels, 128, 30));
    assert!(shadow < ground, "shadow {shadow} ground {ground}");
}

#[test]
fn a_zoomed_out_painting_keeps_its_usual_surround() {
    let gpu = GpuContext::new().unwrap();
    let pixels = shown(&gpu, &mut blank(&gpu, Primer::WhiteGesso), ZOOMED_OUT);
    for (x, y) in [(20, 16), (128, 30), (63, 64)] {
        let here = texel(&pixels, x, y);
        let close = here
            .iter()
            .zip([192u8, 191, 188])
            .all(|(shown, expected)| shown.abs_diff(expected) <= 1);
        assert!(close, "{x},{y}: {here:?}");
    }
}
