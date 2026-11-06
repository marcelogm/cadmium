use cadmium_engine::brush::{BrushSpec, factory_brushes};
use cadmium_engine::canvas::{Primer, Weave};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::params::SimParams;
use cadmium_engine::present::readback_rgba8;
use cadmium_studio::{
    DemoStroke, OrbitCamera, Phase, PlayerRequest, SceneLayers, SceneRenderer, SceneRequest,
    StudioPaint, StudioPlayer, samples,
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
    player_with(gpu, &brush())
}

fn player_with(gpu: &GpuContext, brush: &BrushSpec) -> StudioPlayer {
    let paint = paint();
    let request = PlayerRequest {
        brush,
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

/// A player halfway through its stroke: the pen is down, brush in hand.
fn mid_stroke(gpu: &GpuContext, brush: &BrushSpec) -> StudioPlayer {
    let mut player = player_with(gpu, brush);
    let stroke = samples(player.stroke(), player.stroke_box());
    let stroke_ms = stroke.last().unwrap().time_ms;
    player.run_until(gpu, f64::from(stroke_ms) / 2.0).unwrap();
    assert_eq!(player.phase(), Phase::Stroke);
    assert!(player.brush_visible());
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

/// The request with the brush layer on or off.
fn brush_layer(request: SceneRequest<'_>, brush: bool) -> SceneRequest<'_> {
    SceneRequest {
        layers: SceneLayers { brush },
        ..request
    }
}

/// The scene rendered with the brush layer and without it.
fn with_and_without_brush(gpu: &GpuContext, request: SceneRequest<'_>) -> (Vec<u8>, Vec<u8>) {
    let mut scene = SceneRenderer::new(gpu);
    let on = render_rgba(gpu, &mut scene, brush_layer(request, true));
    let off = render_rgba(gpu, &mut scene, brush_layer(request, false));
    (on, off)
}

fn differs_at(a: &[u8], b: &[u8], width: u32, at: (u32, u32)) -> bool {
    pixel(a, width, at) != pixel(b, width, at)
}

#[test]
fn the_brush_covers_part_of_the_view_mid_stroke() {
    let gpu = GpuContext::new().unwrap();
    let size = (320, 200);
    let player = mid_stroke(&gpu, &brush());
    let camera = camera(&player, size);
    let (on, off) = with_and_without_brush(&gpu, request(&player, &camera, size));
    let differing = on
        .as_chunks::<4>()
        .0
        .iter()
        .zip(off.as_chunks::<4>().0)
        .filter(|(a, b)| a != b)
        .count();
    let share = differing as f64 / f64::from(size.0 * size.1);
    assert!((0.002..=0.5).contains(&share), "brush covers {share}");
}

/// The scene mid-stroke with the brush and without it, and how it was seen.
struct BrushShot {
    camera: OrbitCamera,
    size: (u32, u32),
    on: Vec<u8>,
    off: Vec<u8>,
}

impl BrushShot {
    fn of(gpu: &GpuContext, player: &StudioPlayer) -> BrushShot {
        let size = (320, 200);
        let camera = camera(player, size);
        let (on, off) = with_and_without_brush(gpu, request(player, &camera, size));
        BrushShot {
            camera,
            size,
            on,
            off,
        }
    }

    /// Whether the brush changed a pixel within one of where `point`
    /// (canvas px) projects.
    fn drawn_around(&self, point: glam::Vec3) -> bool {
        let ndc = self.camera.view_proj().project_point3(point);
        let (w, h) = (self.size.0 as f32, self.size.1 as f32);
        let x = ((ndc.x + 1.0) / 2.0 * w) as i64;
        let y = ((1.0 - ndc.y) / 2.0 * h) as i64;
        let inside = |(px, py): &(i64, i64)| {
            (0..i64::from(self.size.0)).contains(px) && (0..i64::from(self.size.1)).contains(py)
        };
        (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| (x + dx, y + dy)))
            .filter(inside)
            .any(|(px, py)| differs_at(&self.on, &self.off, self.size.0, (px as u32, py as u32)))
    }
}

#[test]
fn the_brush_is_drawn_where_its_roots_project() {
    let gpu = GpuContext::new().unwrap();
    let player = mid_stroke(&gpu, &brush());
    let snapshot = player.session().engine.bristle_snapshot(&gpu).unwrap();
    let roots = snapshot
        .chains_px
        .iter()
        .map(|chain| glam::Vec3::from(chain[0]));
    let centroid = roots.sum::<glam::Vec3>() / snapshot.chains_px.len() as f32;
    assert!(BrushShot::of(&gpu, &player).drawn_around(centroid));
}

#[test]
fn the_hairs_are_drawn_out_to_their_tips() {
    let gpu = GpuContext::new().unwrap();
    let player = mid_stroke(&gpu, &brush());
    let snapshot = player.session().engine.bristle_snapshot(&gpu).unwrap();
    let shot = BrushShot::of(&gpu, &player);
    let tips = &snapshot.chains_px;
    let drawn = tips
        .iter()
        .filter(|chain| shot.drawn_around(glam::Vec3::from(*chain.last().unwrap())))
        .count();
    assert!(
        drawn * 2 > tips.len(),
        "{drawn} of {} tips drawn",
        tips.len()
    );
}

#[test]
fn the_handle_rises_up_the_brush_axis() {
    let gpu = GpuContext::new().unwrap();
    let player = mid_stroke(&gpu, &brush());
    let snapshot = player.session().engine.bristle_snapshot(&gpu).unwrap();
    let radius = player.session().engine.bristle_view().ferrule_radius_px();
    let ferrule = glam::Vec3::from(snapshot.ferrule_px);
    // 4.5 ferrule radii up the axis is inside the handle (1.5 to 5.5); as far
    // down it lies under the canvas, clear of the hairs, where nothing shows.
    let reach = glam::Vec3::from(snapshot.axis) * (4.5 * radius);
    let shot = BrushShot::of(&gpu, &player);
    assert!(shot.drawn_around(ferrule + reach), "no handle up the axis");
    assert!(
        !shot.drawn_around(ferrule - reach),
        "a handle down the axis"
    );
}

#[test]
fn a_brush_off_show_is_not_drawn() {
    let gpu = GpuContext::new().unwrap();
    let size = (320, 200);
    for player in [player(&gpu), painted(&gpu)] {
        assert!(!player.brush_visible());
        let camera = camera(&player, size);
        let (on, off) = with_and_without_brush(&gpu, request(&player, &camera, size));
        assert!(on == off, "a hidden brush is drawn at {:?}", player.phase());
    }
}

#[test]
fn the_brush_renders_the_same_bytes() {
    let gpu = GpuContext::new().unwrap();
    let size = (320, 200);
    let player = mid_stroke(&gpu, &brush());
    let camera = camera(&player, size);
    let request = brush_layer(request(&player, &camera, size), true);
    let mut scene = SceneRenderer::new(&gpu);
    let first = render_rgba(&gpu, &mut scene, request);
    let second = render_rgba(&gpu, &mut scene, request);
    assert!(first == second, "a re-render differs");
}

#[test]
fn the_brushes_are_written_for_eyeballing() {
    let Ok(dir) = std::env::var(OUTPUT_DIR_VARIABLE) else {
        return;
    };
    let dir = std::path::Path::new(&dir);
    let gpu = GpuContext::new().unwrap();
    let size = (640, 400);
    let shown = [
        ("round", "Round · Synthetic"),
        ("flat", "Flat · Hog Bristle"),
        ("fan", "Fan · Hog Bristle"),
        ("mop", "Mop · Squirrel"),
    ];
    let mut scene = SceneRenderer::new(&gpu);
    for (file, name) in shown {
        let brush = factory_brushes()
            .into_iter()
            .find(|b| b.name == name)
            .unwrap();
        let player = mid_stroke(&gpu, &brush);
        let camera = camera(&player, size);
        let rgba = render_rgba(&gpu, &mut scene, request(&player, &camera, size));
        write_png(dir, &format!("scene-{file}.png"), size, &rgba);
    }
}
