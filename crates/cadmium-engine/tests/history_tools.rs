pub mod support;

use cadmium_engine::{
    canvas::StateTexture,
    gpu::GpuContext,
    history::bytes_per_tile_all_textures,
    session::PaintSession,
    stroke::{StrokeSample, Tool},
};
use support::{drag, line, session, stroke, without_wgpu_errors};

const WET_ROWS_PX: [f64; 3] = [96.0, 128.0, 160.0];
const CROSSING_TOOLS: [Tool; 5] = [
    Tool::Brush,
    Tool::Knife,
    Tool::Blender,
    Tool::Rag,
    Tool::Pencil,
];
const WIDE_DIAMETER_PX: f32 = 160.0;
const PATH_PRESSURE: f32 = 0.9;
const PATH_TILT_DEG: f32 = 35.0;
const DIAGONAL_SAMPLES: u32 = 24;
const DIAGONAL_FROM: [f64; 2] = [90.0, 50.0];
const DIAGONAL_TO: [f64; 2] = [420.0, 200.0];
/// Placed so a blade lagging behind the turn reaches a tile before the dabs' headings do.
const ARC_CENTRE: [f64; 2] = [272.0, 176.0];
const ARC_RADIUS_PX: f64 = 70.0;
/// Few samples round the arc: each corner turns the heading at once.
const ARC_SAMPLES: u32 = 6;
/// Tiles of the 512×256 canvas, saved before and after by each step of every tool: a
/// history that drops no step while the strokes are undone one by one.
const ROOMY_HISTORY_TILES: u64 = 8 * 4 * 2 * CROSSING_TOOLS.len() as u64;

fn snapshot(gpu: &GpuContext, s: &PaintSession) -> Vec<Vec<u8>> {
    StateTexture::PAINT_STATE
        .iter()
        .map(|t| s.canvas.read_bytes(gpu, *t).unwrap())
        .collect()
}

fn wet_ground(gpu: &GpuContext) -> PaintSession {
    let mut s = session(gpu);
    for y in WET_ROWS_PX {
        s.reload_brush(gpu);
        stroke(gpu, &mut s, y, 40.0, 470.0);
    }
    s
}

fn sample(at: [f64; 2], step: u32) -> StrokeSample {
    StrokeSample {
        x: at[0],
        y: at[1],
        pressure: PATH_PRESSURE,
        tilt_x_deg: PATH_TILT_DEG,
        tilt_y_deg: 0.0,
        time_ms: step * 16,
    }
}

fn diagonal() -> Vec<StrokeSample> {
    (0..=DIAGONAL_SAMPLES)
        .map(|i| {
            let along = f64::from(i) / f64::from(DIAGONAL_SAMPLES);
            let at = [0, 1].map(|axis| {
                DIAGONAL_FROM[axis] + (DIAGONAL_TO[axis] - DIAGONAL_FROM[axis]) * along
            });
            sample(at, i)
        })
        .collect()
}

fn arc() -> Vec<StrokeSample> {
    (0..=ARC_SAMPLES)
        .map(|i| {
            let angle = std::f64::consts::PI * f64::from(i) / f64::from(ARC_SAMPLES);
            let at = [
                ARC_CENTRE[0] + ARC_RADIUS_PX * angle.cos(),
                ARC_CENTRE[1] - ARC_RADIUS_PX * angle.sin(),
            ];
            sample(at, i)
        })
        .collect()
}

#[test]
fn undoing_a_stroke_of_any_tool_across_wet_paint_restores_every_state_texture() {
    let gpu = GpuContext::new().unwrap();
    for tool in CROSSING_TOOLS {
        let mut s = wet_ground(&gpu);
        let before = snapshot(&gpu, &s);
        s.brush.tool = tool;
        drag(&gpu, &mut s, &line(128.0, 60.0, 450.0, 0.9, 35.0));
        assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)), "{tool:?}");
        assert!(snapshot(&gpu, &s) == before, "{tool:?}");
    }
}

#[test]
fn undoing_a_wide_stroke_that_turns_or_runs_diagonally_restores_every_state_texture() {
    let gpu = GpuContext::new().unwrap();
    for tool in CROSSING_TOOLS {
        for (name, path) in [("diagonal", diagonal()), ("arc", arc())] {
            let mut s = wet_ground(&gpu);
            s.brush.tool = tool;
            s.brush.geometry.max_diameter_px = WIDE_DIAMETER_PX;
            s.resize_tip();
            let before = snapshot(&gpu, &s);
            drag(&gpu, &mut s, &path);
            assert!(
                without_wgpu_errors(&gpu, || s.undo(&gpu)),
                "{tool:?} {name}"
            );
            assert!(snapshot(&gpu, &s) == before, "{tool:?} {name}");
        }
    }
}

#[test]
fn undoing_several_overlapping_strokes_steps_back_through_every_earlier_state() {
    let gpu = GpuContext::new().unwrap();
    let mut s = wet_ground(&gpu);
    s.history
        .refit(&gpu, ROOMY_HISTORY_TILES * bytes_per_tile_all_textures());
    let mut states = vec![snapshot(&gpu, &s)];
    for tool in CROSSING_TOOLS {
        s.brush.tool = tool;
        s.reload_brush(&gpu);
        drag(&gpu, &mut s, &line(120.0, 60.0, 450.0, 0.9, 35.0));
        states.push(snapshot(&gpu, &s));
    }
    states.pop();
    while let Some(earlier) = states.pop() {
        assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
        assert!(snapshot(&gpu, &s) == earlier, "{} steps left", states.len());
    }
}

#[test]
fn undoing_before_any_frame_was_drawn_restores_the_canvas() {
    let gpu = GpuContext::new().unwrap();
    let mut s = wet_ground(&gpu);
    let before = snapshot(&gpu, &s);
    let samples = line(128.0, 60.0, 450.0, 0.9, 35.0);
    s.begin_stroke(samples[0]);
    s.add_samples(&samples[1..]);
    s.end_stroke();
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert!(snapshot(&gpu, &s) == before);
}

#[test]
fn a_stroke_of_any_tool_is_undoable_as_soon_as_its_last_frame_is_drawn() {
    let gpu = GpuContext::new().unwrap();
    for tool in CROSSING_TOOLS {
        let mut s = wet_ground(&gpu);
        s.history.clear();
        s.brush.tool = tool;
        drag(&gpu, &mut s, &line(128.0, 60.0, 450.0, 0.9, 35.0));
        assert!(s.history.can_undo(), "{tool:?}");
    }
}
