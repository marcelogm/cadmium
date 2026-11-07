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
        ShadeRequest {
            params: &SimParams::default(),
            mode: ViewMode::Color,
        },
        &mut enc,
    );
    presenter.encode_present(
        gpu,
        PresentRequest::of_canvas(canvas, view),
        &target,
        &mut enc,
    );
    gpu.queue.submit([enc.finish()]);
    readback_rgba8(gpu, &target).unwrap()
}

const IDENTITY_VIEW: ViewTransform = ViewTransform {
    origin_x: 0.0,
    origin_y: 0.0,
    zoom: 1.0,
    rotation: 0.0,
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
            rotation: 0.0,
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

const CANVAS_SIZE: u32 = 128;

fn present_to_rgba8(
    gpu: &GpuContext,
    presenter: &Presenter,
    request: PresentRequest<'_>,
    w: u32,
    h: u32,
) -> Vec<u8> {
    let target = presenter.create_target(gpu, w, h);
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    presenter.encode_present(gpu, request, &target, &mut enc);
    gpu.queue.submit([enc.finish()]);
    readback_rgba8(gpu, &target).unwrap()
}

#[test]
fn presenting_the_display_view_directly_matches_presenting_the_canvas() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = blank(&gpu);
    let n = (CANVAS_SIZE * CANVAS_SIZE) as usize;
    let mut pig = vec![0.0f32; n * 4];
    for i in 0..n {
        pig[i * 4] = 1.0;
    }
    canvas.write_f32(&gpu, StateTexture::WetPig0, &pig);
    canvas.write_f32(&gpu, StateTexture::WetVol, &vec![200.0; n]);
    canvas.dirty.mark_all();
    let via_canvas = shade_and_present(&gpu, &mut canvas, IDENTITY_VIEW, CANVAS_SIZE, CANVAS_SIZE);
    let presenter = Presenter::new(&gpu);
    let request = PresentRequest {
        source: canvas.view(StateTexture::Display),
        view: IDENTITY_VIEW,
        plate: false,
    };
    let via_view = present_to_rgba8(&gpu, &presenter, request, CANVAS_SIZE, CANVAS_SIZE);
    assert_eq!(via_canvas, via_view);
}

const SOURCE_SIZE: (u32, u32) = (64, 32);
const TARGET_SIZE: (u32, u32) = (96, 48);
const F16_ONE: u16 = 0x3C00;
const F16_HALF: u16 = 0x3800;
const F16_NEG_TENTH: u16 = 0xAE66;
const BACKGROUND_SRGB8: [u8; 3] = [192, 191, 188];

fn solid_source(gpu: &GpuContext, rgb: [u16; 3]) -> wgpu::TextureView {
    let (w, h) = SOURCE_SIZE;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("solid_source"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let texel: Vec<u8> = [rgb[0], rgb[1], rgb[2], F16_ONE]
        .iter()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    let data = texel.repeat((w * h) as usize);
    gpu.queue.write_texture(
        texture.as_image_copy(),
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 8),
            rows_per_image: Some(h),
        },
        texture.size(),
    );
    texture.create_view(&Default::default())
}

fn present_solid(gpu: &GpuContext, rgb: [u16; 3]) -> Vec<u8> {
    let source = solid_source(gpu, rgb);
    let presenter = Presenter::new(gpu);
    let request = PresentRequest {
        source: &source,
        view: IDENTITY_VIEW,
        plate: false,
    };
    present_to_rgba8(gpu, &presenter, request, TARGET_SIZE.0, TARGET_SIZE.1)
}

fn texel(px: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * TARGET_SIZE.0 + x) * 4) as usize;
    [px[i], px[i + 1], px[i + 2]]
}

#[test]
fn a_standalone_linear_source_is_encoded_and_surrounded_by_background() {
    let gpu = GpuContext::new().unwrap();
    let px = present_solid(&gpu, [F16_HALF; 3]);
    let expected = (255.0_f32 * (1.055 * 0.5_f32.powf(1.0 / 2.4) - 0.055)).round() as i32;
    for c in texel(&px, 10, 10) {
        assert!((c as i32 - expected).abs() <= 1, "{c} vs {expected}");
    }
    assert_eq!(texel(&px, 80, 40), BACKGROUND_SRGB8);
}

#[test]
fn an_out_of_gamut_source_is_compressed_toward_gray_not_clipped() {
    let gpu = GpuContext::new().unwrap();
    let px = present_solid(&gpu, [F16_NEG_TENTH, F16_HALF, F16_HALF]);
    let [r, g, b] = texel(&px, 10, 10);
    // Clipping alone would leave g = b = 0.5 linear (sRGB 188); compression pulls them in.
    assert!(r <= 1, "red {r}");
    assert!(g.abs_diff(b) <= 1, "{g} {b}");
    assert!((180..=186).contains(&g), "green {g}");
}

#[test]
fn shading_consumes_dirty_tiles() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = blank(&gpu);
    let _ = shade_and_present(&gpu, &mut canvas, IDENTITY_VIEW, 16, 16);
    assert!(canvas.dirty.is_empty());
}

#[test]
fn impasto_edge_facing_the_light_is_brighter_than_the_edge_in_shadow() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = blank(&gpu);
    let side = 128usize;
    let inside = |x: usize, y: usize| (32..96).contains(&x) && (32..96).contains(&y);
    let mut pig = vec![0.0f32; side * side * 4];
    let mut vol = vec![0.0f32; side * side];
    for y in 0..side {
        for x in (0..side).filter(|&x| inside(x, y)) {
            pig[(y * side + x) * 4] = 1.0;
            vol[y * side + x] = 200.0;
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

#[test]
fn a_quarter_turn_swings_the_canvas_around_the_view_origin() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = blank(&gpu);
    let turned = ViewTransform {
        rotation: std::f32::consts::FRAC_PI_2,
        ..IDENTITY_VIEW
    };
    let px = shade_and_present(&gpu, &mut canvas, turned, TARGET_SIZE.0, TARGET_SIZE.1);
    assert_eq!(texel(&px, 48, 24), BACKGROUND_SRGB8);
    let shifted_into_view = ViewTransform {
        origin_y: 64.0,
        ..turned
    };
    let px = shade_and_present(
        &gpu,
        &mut canvas,
        shifted_into_view,
        TARGET_SIZE.0,
        TARGET_SIZE.1,
    );
    assert_ne!(texel(&px, 48, 24), BACKGROUND_SRGB8);
}
