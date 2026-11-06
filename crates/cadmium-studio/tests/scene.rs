use cadmium_engine::brush::{BrushSpec, factory_brushes};
use cadmium_engine::canvas::{Primer, Weave};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::params::SimParams;
use cadmium_engine::present::readback_rgba8;
use cadmium_studio::{
    DemoStroke, OrbitCamera, Phase, PlayerRequest, SceneLayers, SceneRenderer, SceneRequest,
    StudioPaint, StudioPlayer,
};

/// Where eyeballing images go, if set; never into the repo.
const OUTPUT_DIR_VARIABLE: &str = "CADMIUM_SCENE_OUTPUT";
const LAYERS: SceneLayers = SceneLayers { brush: true };

fn paint() -> StudioPaint {
    StudioPaint {
        slots: vec!["ultramarine_blue".to_string()],
        primer: Primer::WhiteGesso,
        weave: Weave::Smooth,
        recipe: vec![(0, 1.0)],
        load: 1.0,
        medium: 0.1,
        params: SimParams::default(),
    }
}

/// A small brush keeps the canvas, and so each frame, cheap.
fn brush() -> BrushSpec {
    BrushSpec {
        size_px: 12.0,
        ..factory_brushes()[0].clone()
    }
}

fn player(gpu: &GpuContext) -> StudioPlayer {
    let (brush, paint) = (brush(), paint());
    let request = PlayerRequest {
        brush: &brush,
        paint: &paint,
        stroke: DemoStroke::SCurve,
    };
    StudioPlayer::new(gpu, request).unwrap()
}

/// A player whose stroke is done and on show.
fn painted(gpu: &GpuContext) -> StudioPlayer {
    let mut player = player(gpu);
    player.run_until(gpu, player.loop_ms()).unwrap();
    assert_eq!(player.phase(), Phase::Pause);
    player
}

fn camera(player: &StudioPlayer, (w, h): (u32, u32)) -> OrbitCamera {
    OrbitCamera::framing(player.stroke_box(), player.reach_px(), w as f32 / h as f32)
}

fn request<'a>(
    player: &'a StudioPlayer,
    camera: &'a OrbitCamera,
    size: (u32, u32),
) -> SceneRequest<'a> {
    SceneRequest {
        player,
        camera,
        size,
        layers: LAYERS,
    }
}

/// The scene as straight sRGB RGBA8 at the request's size.
fn render_rgba(gpu: &GpuContext, scene: &mut SceneRenderer, request: SceneRequest<'_>) -> Vec<u8> {
    let presenter = &request.player.session().presenter;
    let target = presenter.create_target(gpu, request.size.0, request.size.1);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let frame = scene.render(gpu, request, &mut encoder);
    presenter.encode_present(gpu, frame.present_request(), &target, &mut encoder);
    gpu.queue.submit([encoder.finish()]);
    readback_rgba8(gpu, &target).unwrap()
}

fn write_png(dir: &std::path::Path, name: &str, (w, h): (u32, u32), rgba: &[u8]) {
    let file = std::fs::File::create(dir.join(name)).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(rgba).unwrap();
}

/// The RGB of the pixel at (`x`, `y`) of an RGBA8 image `width` wide.
fn pixel(rgba: &[u8], width: u32, (x, y): (u32, u32)) -> [u8; 3] {
    let at = ((y * width + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2]]
}

/// Whether every channel is within `tolerance` of the other's.
fn near(a: [u8; 3], b: [u8; 3], tolerance: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tolerance)
}

/// The backdrop as presented: the clear colour, sRGB-encoded.
fn backdrop_srgb8() -> [u8; 3] {
    let encode = |linear: f32| (1.055 * linear.powf(1.0 / 2.4) - 0.055) * 255.0;
    [0.02, 0.02, 0.025].map(|c: f32| encode(c).round() as u8)
}

#[test]
fn the_same_state_renders_the_same_bytes() {
    let gpu = GpuContext::new().unwrap();
    let size = (320, 200);
    let player = painted(&gpu);
    let camera = camera(&player, size);
    let mut scene = SceneRenderer::new(&gpu);
    let request = request(&player, &camera, size);
    let first = render_rgba(&gpu, &mut scene, request);
    let second = render_rgba(&gpu, &mut scene, request);
    let fresh = render_rgba(&gpu, &mut SceneRenderer::new(&gpu), request);
    assert_eq!(first.len(), (size.0 * size.1 * 4) as usize);
    assert!(first == second, "a re-render differs");
    assert!(first == fresh, "another renderer differs");
}

#[test]
fn the_plane_shows_the_canvas() {
    let gpu = GpuContext::new().unwrap();
    let size = (320, 200);
    let blank = player(&gpu);
    let painted = painted(&gpu);
    let camera = camera(&painted, size);
    let mut scene = SceneRenderer::new(&gpu);
    let before = render_rgba(&gpu, &mut scene, request(&blank, &camera, size));
    let after = render_rgba(&gpu, &mut scene, request(&painted, &camera, size));
    assert!(before != after, "the stroke does not show");
}

#[test]
fn the_backdrop_frames_a_canvas_of_gesso() {
    let gpu = GpuContext::new().unwrap();
    let size = (320, 200);
    let blank = player(&gpu);
    let camera = camera(&blank, size);
    let rgba = render_rgba(
        &gpu,
        &mut SceneRenderer::new(&gpu),
        request(&blank, &camera, size),
    );
    // The reset view looks past the canvas's far corners onto the backdrop;
    // ±1 for the presenter's dither.
    let corner = pixel(&rgba, size.0, (0, 0));
    assert!(near(corner, backdrop_srgb8(), 1), "corner {corner:?}");
    let flat = blank.rgba(&gpu).unwrap();
    let canvas = &blank.session().canvas.spec;
    let gesso = pixel(&flat, canvas.width, (canvas.width / 2, canvas.height / 2));
    let centre = pixel(&rgba, size.0, (size.0 / 2, size.1 / 2));
    assert!(
        near(centre, gesso, 6),
        "centre {centre:?} vs gesso {gesso:?}"
    );
}

#[test]
fn the_scene_is_written_for_eyeballing() {
    let Ok(dir) = std::env::var(OUTPUT_DIR_VARIABLE) else {
        return;
    };
    let dir = std::path::Path::new(&dir);
    let gpu = GpuContext::new().unwrap();
    let size = (640, 400);
    let player = painted(&gpu);
    let camera = camera(&player, size);
    let request = request(&player, &camera, size);
    let rgba = render_rgba(&gpu, &mut SceneRenderer::new(&gpu), request);
    write_png(dir, "scene-plane.png", size, &rgba);
    let canvas = &player.session().canvas.spec;
    let flat = player.rgba(&gpu).unwrap();
    write_png(
        dir,
        "scene-canvas.png",
        (canvas.width, canvas.height),
        &flat,
    );
}
