use cadmium_engine::brush::{BrushSpec, factory_brushes};
use cadmium_engine::canvas::{Primer, Weave};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::params::SimParams;
use cadmium_engine::present::{PresentRequest, ViewTransform, readback_rgba8};
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

/// The scene as straight sRGB RGBA8 at `size`.
fn render_rgba(
    gpu: &GpuContext,
    scene: &mut SceneRenderer,
    player: &StudioPlayer,
    camera: &OrbitCamera,
    size: (u32, u32),
) -> Vec<u8> {
    let presenter = &player.session().presenter;
    let target = presenter.create_target(gpu, size.0, size.1);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let request = SceneRequest {
        player,
        camera,
        size,
        layers: LAYERS,
    };
    // A view is a handle: the clone frees `scene` for `supersample`.
    let view = scene.render(gpu, request, &mut encoder).clone();
    let transform = ViewTransform {
        origin_x: 0.0,
        origin_y: 0.0,
        zoom: 1.0 / scene.supersample(),
    };
    let present = PresentRequest {
        source: &view,
        view: transform,
    };
    presenter.encode_present(gpu, present, &target, &mut encoder);
    gpu.queue.submit([encoder.finish()]);
    readback_rgba8(gpu, &target).unwrap()
}

fn write_png(name: &str, (w, h): (u32, u32), rgba: &[u8]) {
    let Ok(dir) = std::env::var(OUTPUT_DIR_VARIABLE) else {
        return;
    };
    let file = std::fs::File::create(std::path::Path::new(&dir).join(name)).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(rgba).unwrap();
}

#[test]
fn the_same_state_renders_the_same_bytes() {
    let gpu = GpuContext::new().unwrap();
    let size = (320, 200);
    let player = painted(&gpu);
    let camera = camera(&player, size);
    let mut scene = SceneRenderer::new(&gpu);
    let first = render_rgba(&gpu, &mut scene, &player, &camera, size);
    let second = render_rgba(&gpu, &mut scene, &player, &camera, size);
    let fresh = render_rgba(&gpu, &mut SceneRenderer::new(&gpu), &player, &camera, size);
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
    let before = render_rgba(&gpu, &mut scene, &blank, &camera, size);
    let after = render_rgba(&gpu, &mut scene, &painted, &camera, size);
    assert!(before != after, "the stroke does not show");
}

#[test]
fn the_scene_is_written_for_eyeballing() {
    let gpu = GpuContext::new().unwrap();
    let size = (640, 400);
    let player = painted(&gpu);
    let camera = camera(&player, size);
    let rgba = render_rgba(&gpu, &mut SceneRenderer::new(&gpu), &player, &camera, size);
    write_png("scene-plane.png", size, &rgba);
    let canvas = &player.session().canvas.spec;
    let flat = player.rgba(&gpu).unwrap();
    write_png("scene-canvas.png", (canvas.width, canvas.height), &flat);
}
