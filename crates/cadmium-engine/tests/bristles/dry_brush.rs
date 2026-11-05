use cadmium_engine::canvas::StateTexture;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;

use super::look::{HEIGHT, ROW, WIDTH, straight, volume};
use crate::support::{drag, session, write_preview};

const RIDGE_CENTRES_X: [usize; 3] = [200, 256, 312];
const RIDGE_HEIGHT_UM: f32 = 150.0;
const WALL_WIDTH_PX: usize = 6;
const HOLLOW_WIDTH_PX: usize = 8;
const RIDGE_ROWS: std::ops::Range<usize> = 16..240;
const OPEN_VALLEY_MARGIN_PX: usize = 6;
const BESIDE_RIDGES_X: std::ops::Range<usize> = 170..342;
const DRY_BRUSH_LOAD: f32 = 0.1;
const DRY_BRUSH_PRESSURE: f32 = 0.25;
const DRY_BRUSH_BAND_PX: usize = 8;
const MIN_RIDGE_TO_VALLEY_RATIO: f64 = 1.6;
const MAX_HOLLOW_SHARE: f64 = 0.85;

#[derive(Clone, Copy, PartialEq)]
enum Ground {
    Wall,
    Hollow,
    OpenValley,
    Beside,
}

fn ground_at(x: usize) -> Ground {
    let half_hollow = HOLLOW_WIDTH_PX / 2;
    let half_ridge = half_hollow + WALL_WIDTH_PX;
    let nearest = RIDGE_CENTRES_X
        .iter()
        .map(|&centre| x.abs_diff(centre))
        .min()
        .expect("there are ridges");
    if nearest < half_hollow {
        Ground::Hollow
    } else if nearest < half_ridge {
        Ground::Wall
    } else if nearest >= half_ridge + OPEN_VALLEY_MARGIN_PX {
        Ground::OpenValley
    } else {
        Ground::Beside
    }
}

fn dried_ridges() -> Vec<f32> {
    (0..WIDTH * HEIGHT)
        .map(|pixel| {
            let on_wall = ground_at(pixel % WIDTH) == Ground::Wall;
            if on_wall && RIDGE_ROWS.contains(&(pixel / WIDTH)) {
                RIDGE_HEIGHT_UM
            } else {
                0.0
            }
        })
        .collect()
}

fn mean_on(volume: &[f32], ground: Ground) -> f64 {
    let row = ROW as usize;
    let pixels: Vec<usize> = (row - DRY_BRUSH_BAND_PX..=row + DRY_BRUSH_BAND_PX)
        .flat_map(|y| BESIDE_RIDGES_X.map(move |x| (y, x)))
        .filter(|&(_, x)| ground_at(x) == ground)
        .map(|(y, x)| y * WIDTH + x)
        .collect();
    pixels
        .iter()
        .map(|&pixel| f64::from(volume[pixel]))
        .sum::<f64>()
        / pixels.len().max(1) as f64
}

fn dry_brush_stroke(gpu: &GpuContext) -> PaintSession {
    let mut painting = session(gpu);
    painting
        .canvas
        .write_f32(gpu, StateTexture::DryHeight, &dried_ridges());
    painting.brush.load = DRY_BRUSH_LOAD;
    painting.reload_brush(gpu);
    drag(gpu, &mut painting, &straight(ROW, |_| DRY_BRUSH_PRESSURE));
    painting
}

#[test]
fn a_dry_brush_loads_the_ridges_it_rides_and_bridges_the_narrow_hollows() {
    let gpu = GpuContext::new().unwrap();
    let painting = dry_brush_stroke(&gpu);
    write_preview(&gpu, &painting, "bristle-dry-brush.png");
    let volume = volume(&gpu, &painting);
    let on_ridge_um = mean_on(&volume, Ground::Wall);
    let hollow_um = mean_on(&volume, Ground::Hollow);
    let open_valley_um = mean_on(&volume, Ground::OpenValley);
    eprintln!(
        "ridges {RIDGE_HEIGHT_UM} um tall: {on_ridge_um:.3} um on them, {hollow_um:.3} um in the \
         hollows between them, {open_valley_um:.3} um in the open valleys"
    );
    assert!(
        on_ridge_um >= MIN_RIDGE_TO_VALLEY_RATIO * open_valley_um,
        "the valleys took nearly as much paint as the ridges: {on_ridge_um:.3} vs {open_valley_um:.3}"
    );
    assert!(
        hollow_um <= MAX_HOLLOW_SHARE * open_valley_um,
        "the hairs dropped into the hollows between ridges: {hollow_um:.3} vs {open_valley_um:.3}"
    );
}
