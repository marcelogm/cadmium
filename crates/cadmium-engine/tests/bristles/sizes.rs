use std::collections::{BTreeSet, VecDeque};

use cadmium_engine::brush::HeadSpec;
use cadmium_engine::canvas::StateTexture;
use cadmium_engine::engine::{BristleSnapshot, PaintEngine};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::{Dab, StrokeSample};

use crate::hull::{convex_hull, distance_to_hull, touching_segment_ends};
use crate::support::{drag, line, session, spec, without_wgpu_errors, write_png};

const TINY_DIAMETER_PX: f32 = 2.0;
const LARGEST_DIAMETER_PX: f32 = 500.0;
const LARGE_CANVAS_PX: usize = 2048;
const TINY_ROW_PX: f64 = 128.0;
const TINY_FROM_PX: f64 = 60.0;
const TINY_TO_PX: f64 = 450.0;
const TINY_PRESSURE: f32 = 0.8;
const TINY_TILT_DEG: f32 = 20.0;
const TINY_SETTLE_PX: usize = 10;
const MIN_MARKED_COLUMN_SHARE: f64 = 0.85;
const VISIBLE_UM: f32 = 0.5;
const PAINTED_UM: f32 = 1e-3;
const MAX_ISLAND_PX2: usize = 2;
const FULL_PRESSURE: f32 = 1.0;
const LEANING_TILT_DEG: f32 = 50.0;
const PREVIEW_GAIN: f32 = 16.0;
const WET_STATE: [StateTexture; 3] = [
    StateTexture::WetVol,
    StateTexture::WetPig0,
    StateTexture::WetPig1,
];

fn largest_brush_session(gpu: &GpuContext) -> PaintSession {
    let side = LARGE_CANVAS_PX as u32;
    let mut s = without_wgpu_errors(gpu, || PaintSession::new(gpu, spec(side, side)).unwrap());
    s.brush.geometry.max_diameter_px = LARGEST_DIAMETER_PX;
    s.reload_brush(gpu);
    s
}

fn centre_press(tilt_x_deg: f32) -> StrokeSample {
    let centre = (LARGE_CANVAS_PX / 2) as f64;
    StrokeSample {
        x: centre,
        y: centre,
        pressure: FULL_PRESSURE,
        tilt_x_deg,
        tilt_y_deg: 0.0,
        time_ms: 0,
    }
}

fn wet_volume(gpu: &GpuContext, s: &PaintSession) -> Vec<f32> {
    s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap()
}

fn preview(volume: &[f32]) -> Vec<u8> {
    volume
        .iter()
        .flat_map(|&volume| {
            let shade = 255.0 - (volume * PREVIEW_GAIN).min(255.0);
            [shade as u8, shade as u8, 255, 255]
        })
        .collect()
}

struct Mark {
    marked_column_share: f64,
    mean_width_px: f64,
}

fn tiny_mark(volume: &[f32], width: usize) -> Mark {
    let first = TINY_FROM_PX as usize + TINY_SETTLE_PX;
    let last = TINY_TO_PX as usize - TINY_SETTLE_PX;
    let widths_px: Vec<usize> = (first..=last)
        .map(|x| {
            let column = volume.iter().skip(x).step_by(width);
            column.filter(|&&v| v > VISIBLE_UM).count()
        })
        .collect();
    let columns = widths_px.len() as f64;
    Mark {
        marked_column_share: widths_px.iter().filter(|&&w| w > 0).count() as f64 / columns,
        mean_width_px: widths_px.iter().sum::<usize>() as f64 / columns,
    }
}

fn density_extremes() -> [f32; 2] {
    HeadSpec::DENSITY_RANGE_PER_PX
}

#[test]
fn a_two_pixel_brush_still_leaves_a_mark() {
    let gpu = GpuContext::new().unwrap();
    for density_per_px in density_extremes() {
        let mut s = session(&gpu);
        s.brush.head.density_per_px = density_per_px;
        s.brush.geometry.max_diameter_px = TINY_DIAMETER_PX;
        s.reload_brush(&gpu);
        let samples = line(
            TINY_ROW_PX,
            TINY_FROM_PX,
            TINY_TO_PX,
            TINY_PRESSURE,
            TINY_TILT_DEG,
        );
        drag(&gpu, &mut s, &samples);
        let volume = wet_volume(&gpu, &s);
        let width = s.canvas.spec.width as usize;
        write_png(
            &format!("bristle-tiny-stroke-{density_per_px}.png"),
            width as u32,
            s.canvas.spec.height,
            &preview(&volume),
        );
        let mark = tiny_mark(&volume, width);
        eprintln!(
            "a 2 px brush at {density_per_px} bristles/px marked {:.3} of its stroke, {:.2} px wide",
            mark.marked_column_share, mark.mean_width_px
        );
        assert!(
            mark.marked_column_share >= MIN_MARKED_COLUMN_SHARE,
            "a 2 px brush at {density_per_px} bristles/px marked only {:.3} of its stroke",
            mark.marked_column_share
        );
        assert!(
            mark.mean_width_px >= f64::from(TINY_DIAMETER_PX),
            "a 2 px brush at {density_per_px} bristles/px laid a mark {:.2} px wide",
            mark.mean_width_px
        );
    }
}

fn pixel_centre(index: usize) -> [f32; 2] {
    [
        (index % LARGE_CANVAS_PX) as f32 + 0.5,
        (index / LARGE_CANVAS_PX) as f32 + 0.5,
    ]
}

fn neighbours(index: usize) -> impl Iterator<Item = usize> {
    let side = LARGE_CANVAS_PX as i64;
    let (x, y) = (
        (index % LARGE_CANVAS_PX) as i64,
        (index / LARGE_CANVAS_PX) as i64,
    );
    (-1..=1)
        .flat_map(move |dy| (-1..=1).map(move |dx| (x + dx, y + dy)))
        .filter(move |&(nx, ny)| {
            (nx, ny) != (x, y) && (0..side).contains(&nx) && (0..side).contains(&ny)
        })
        .map(move |(nx, ny)| (ny * side + nx) as usize)
}

fn flood(open: &[bool], seen: &mut [bool], seeds: impl IntoIterator<Item = usize>) -> usize {
    let mut queue: VecDeque<usize> = seeds.into_iter().filter(|&seed| open[seed]).collect();
    for &seed in &queue {
        seen[seed] = true;
    }
    let mut reached = 0;
    while let Some(index) = queue.pop_front() {
        reached += 1;
        let fresh: Vec<usize> = neighbours(index)
            .filter(|&next| open[next] && !seen[next])
            .collect();
        for next in fresh {
            seen[next] = true;
            queue.push_back(next);
        }
    }
    reached
}

fn island_areas_px2(volume: &[f32], hull: &[[f32; 2]]) -> Vec<usize> {
    let open: Vec<bool> = volume.iter().map(|&v| v <= PAINTED_UM).collect();
    let mut seen = vec![false; open.len()];
    let outside =
        (0..open.len()).filter(|&index| distance_to_hull(pixel_centre(index), hull) > 0.0);
    flood(&open, &mut seen, outside);
    let mut islands = Vec::new();
    for index in 0..open.len() {
        if open[index] && !seen[index] {
            islands.push(flood(&open, &mut seen, [index]));
        }
    }
    islands
}

#[test]
fn the_largest_brush_pressed_without_clumping_leaves_no_holes() {
    let gpu = GpuContext::new().unwrap();
    let mut s = largest_brush_session(&gpu);
    s.engine.params_mut().set("bristle_clumping", 0.0);
    drag(&gpu, &mut s, &[centre_press(0.0)]);
    let snapshot = s.engine.bristle_snapshot(&gpu).unwrap();
    let hull = convex_hull(touching_segment_ends(&snapshot));
    let volume = wet_volume(&gpu, &s);
    write_png(
        "bristle-largest-dab.png",
        LARGE_CANVAS_PX as u32,
        LARGE_CANVAS_PX as u32,
        &preview(&volume),
    );
    let painted = volume.iter().filter(|&&v| v > PAINTED_UM).count();
    let islands = island_areas_px2(&volume, &hull);
    let largest = islands.iter().copied().max().unwrap_or(0);
    eprintln!(
        "{} bristles touch; {painted} px painted; {} islands, largest {largest} px²",
        snapshot.touching.iter().filter(|&&t| t).count(),
        islands.len()
    );
    assert!(painted > 0, "the largest brush laid no paint");
    assert!(
        largest < MAX_ISLAND_PX2,
        "an unpainted island of {largest} px² sits inside the pressed dab"
    );
}

fn still_dab(sample: &StrokeSample) -> Dab {
    Dab {
        x: sample.x as f32,
        y: sample.y as f32,
        radius: LARGEST_DIAMETER_PX / 2.0,
        pressure: sample.pressure,
        altitude: std::f32::consts::FRAC_PI_2,
        azimuth: 0.0,
        direction: 0.0,
        speed: 0.0,
        time_ms: 0.0,
    }
}

fn changed_pixels(old: &[u8], new: &[u8]) -> impl Iterator<Item = usize> {
    let bytes_per_pixel = old.len() / (LARGE_CANVAS_PX * LARGE_CANVAS_PX);
    old.chunks_exact(bytes_per_pixel)
        .zip(new.chunks_exact(bytes_per_pixel))
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(pixel, _)| pixel)
}

fn changed_outside(before: &[Vec<u8>], after: &[Vec<u8>], bounds: [f32; 4]) -> usize {
    let [x0, y0, x1, y1] = bounds;
    let inside = |pixel: &usize| {
        let [x, y] = [pixel % LARGE_CANVAS_PX, pixel / LARGE_CANVAS_PX].map(|v| v as f32);
        (x0..=x1).contains(&x) && (y0..=y1).contains(&y)
    };
    let changed: BTreeSet<usize> = before
        .iter()
        .zip(after)
        .flat_map(|(old, new)| changed_pixels(old, new))
        .filter(|pixel| !inside(pixel))
        .collect();
    changed.len()
}

fn clipped_contacts(snapshot: &BristleSnapshot, bounds: [f32; 4]) -> usize {
    let [x0, y0, x1, y1] = bounds;
    let reaches_past = |vertex: &[f32; 3], radius_px: f32| {
        vertex[0] - radius_px < x0
            || vertex[1] - radius_px < y0
            || vertex[0] + radius_px > x1 + 1.0
            || vertex[1] + radius_px > y1 + 1.0
    };
    snapshot
        .chains_px
        .iter()
        .zip(&snapshot.radii_px)
        .flat_map(|(chain, &radius_px)| {
            chain
                .iter()
                .filter(move |vertex| vertex[2] <= radius_px && reaches_past(vertex, radius_px))
        })
        .count()
}

fn wet_state(gpu: &GpuContext, s: &PaintSession) -> Vec<Vec<u8>> {
    WET_STATE
        .iter()
        .map(|&which| s.canvas.read_bytes(gpu, which).unwrap())
        .collect()
}

#[test]
fn the_largest_brush_keeps_the_dab_inside_its_saved_tiles() {
    let gpu = GpuContext::new().unwrap();
    for tilt_x_deg in [0.0, LEANING_TILT_DEG] {
        let mut s = largest_brush_session(&gpu);
        let press = centre_press(tilt_x_deg);
        let bounds = s
            .engine
            .dab_bounds(&s.canvas, &s.brush, &still_dab(&press))
            .unwrap();
        let canvas_last = (LARGE_CANVAS_PX - 1) as f32;
        assert!(
            bounds[0] > 0.0
                && bounds[1] > 0.0
                && bounds[2] < canvas_last
                && bounds[3] < canvas_last,
            "the canvas is too small to see paint leave {bounds:?}"
        );
        let before = wet_state(&gpu, &s);
        drag(&gpu, &mut s, &[press]);
        let after = wet_state(&gpu, &s);
        assert!(
            before != after,
            "the dab at tilt {tilt_x_deg}° changed nothing"
        );
        let snapshot = s.engine.bristle_snapshot(&gpu).unwrap();
        let clipped = clipped_contacts(&snapshot, bounds);
        assert_eq!(
            clipped, 0,
            "{clipped} touching bristle vertices reach past {bounds:?}"
        );
        let outside = changed_outside(&before, &after, bounds);
        eprintln!("tilt {tilt_x_deg}°: bounds {bounds:?}, {outside} px changed outside");
        assert_eq!(
            outside, 0,
            "the dab at tilt {tilt_x_deg}° changed {outside} px outside {bounds:?}"
        );
    }
}
