use super::{DabRegions, FirstRest, Footprint, MAX_SWEEP_PX, Placement, swept_bounds};
use crate::brush::BrushSettings;
use crate::canvas::{TileCoord, tiles_in_rect};
use crate::stroke::{Dab, Tool};

const RESTING_TOOLS: [Tool; 3] = [Tool::Brush, Tool::Knife, Tool::Blender];
/// A blade, a bristle and an ellipse footprint.
const FOOTPRINT_TOOLS: [Tool; 3] = [Tool::Knife, Tool::Brush, Tool::Rag];
const CANVAS_LIMITS: [i32; 2] = [512, 256];
const FERRULE_RADIUS_PX: f32 = 20.0;
const BLADE_TURNS: u8 = 12;
const SWEEP_HEADINGS: u8 = 8;
const SWEEP_LENGTHS_PX: [f32; 3] = [0.0, 0.5 * MAX_SWEEP_PX, MAX_SWEEP_PX];
/// How far past its footprint's reach an off-canvas dab lies: less than a sweep's length,
/// so a sweep from the canvas side still brings its region onto the canvas.
const OFF_CANVAS_GAP_PX: f32 = 6.0;
const SMALL_DAB_RADIUS_PX: f32 = 3.0;
const UPRIGHT_ALTITUDE_RAD: f32 = std::f32::consts::FRAC_PI_2;

fn dab_at(x: f32, y: f32) -> Dab {
    Dab {
        x,
        y,
        radius: SMALL_DAB_RADIUS_PX,
        pressure: 0.8,
        altitude: UPRIGHT_ALTITUDE_RAD,
        azimuth: 0.4,
        direction: 0.7,
        speed: 1.0,
        time_ms: 0.0,
    }
}

fn brush_of(tool: Tool) -> BrushSettings {
    BrushSettings {
        tool,
        ..BrushSettings::default()
    }
}

fn sweeps() -> Vec<[f32; 2]> {
    let headings = (0..SWEEP_HEADINGS)
        .map(|i| (f32::from(i) + 0.5) * std::f32::consts::TAU / f32::from(SWEEP_HEADINGS));
    headings
        .flat_map(|heading| {
            SWEEP_LENGTHS_PX.map(|length| [length * heading.cos(), length * heading.sin()])
        })
        .collect()
}

fn holds(outer: [f32; 4], inner: [f32; 4]) -> bool {
    outer[0] <= inner[0] && outer[1] <= inner[1] && outer[2] >= inner[2] && outer[3] >= inner[3]
}

/// The canvas rectangles the dab dispatches over when swept by `sweep`, one for each turn
/// of a blade that reaches the canvas.
fn dispatched_at_every_turn(tool: Tool, dab: &Dab, sweep: [f32; 2]) -> Vec<[f32; 4]> {
    let brush = brush_of(tool);
    (0..BLADE_TURNS)
        .filter_map(|turn| {
            let orientation = f32::from(turn) * std::f32::consts::PI / f32::from(BLADE_TURNS);
            let turned = Footprint::of(dab, &brush, FERRULE_RADIUS_PX, Some(orientation));
            DabRegions::around(dab, sweep, turned, CANVAS_LIMITS)
        })
        .map(|regions| regions.canvas_rect())
        .collect()
}

/// How many of the dab's sweeps reach the canvas, each checked to dispatch inside the
/// bounds saved for it, at every turn of a blade.
fn sweeps_dispatched_inside_their_saved_bounds(tool: Tool, dab: &Dab) -> usize {
    let unturned = Footprint::of(dab, &brush_of(tool), FERRULE_RADIUS_PX, None);
    let mut dispatched = 0;
    for sweep in sweeps() {
        let swept_from = dab_at(dab.x + sweep[0], dab.y + sweep[1]);
        let saved = swept_bounds(dab, Some(&swept_from), &unturned, CANVAS_LIMITS);
        let written = dispatched_at_every_turn(tool, dab, sweep);
        assert!(
            written
                .iter()
                .all(|written| saved.is_some_and(|saved| holds(saved, *written))),
            "{tool:?} at {:?} swept {sweep:?}: {written:?} outside {saved:?}",
            [dab.x, dab.y]
        );
        dispatched += written.len();
    }
    dispatched
}

#[test]
fn every_swept_dab_dispatches_inside_the_bounds_saved_for_it() {
    for tool in FOOTPRINT_TOOLS {
        let on_canvas = dab_at(200.0, 120.0);
        assert!(sweeps_dispatched_inside_their_saved_bounds(tool, &on_canvas) > 0);
        let unturned = Footprint::of(&on_canvas, &brush_of(tool), FERRULE_RADIUS_PX, None);
        let reach = unturned.canvas_half_extents_at_any_turn();
        let left_of_canvas = dab_at(-(reach[0] + OFF_CANVAS_GAP_PX), 120.0);
        let below_canvas = dab_at(
            200.0,
            CANVAS_LIMITS[1] as f32 + reach[1] + OFF_CANVAS_GAP_PX,
        );
        for off_canvas in [left_of_canvas, below_canvas] {
            assert_eq!(
                swept_bounds(&off_canvas, None, &unturned, CANVAS_LIMITS),
                None
            );
            assert!(
                sweeps_dispatched_inside_their_saved_bounds(tool, &off_canvas) > 0,
                "{tool:?}"
            );
        }
    }
}

#[test]
fn a_sweep_onto_the_canvas_saves_the_tile_only_the_sweep_reaches() {
    let on_canvas = dab_at(10.0, 70.0);
    let off_canvas = dab_at(-10.0, 66.0);
    let footprint = Footprint::of(&off_canvas, &brush_of(Tool::Rag), FERRULE_RADIUS_PX, None);
    assert_eq!(footprint.canvas_half_extents(), [SMALL_DAB_RADIUS_PX; 2]);
    let saved = swept_bounds(&off_canvas, Some(&on_canvas), &footprint, CANVAS_LIMITS).unwrap();
    assert_eq!(saved, [0.0, 61.0, 15.0, 75.0]);
    let [width, height] = CANVAS_LIMITS.map(|limit| limit as u32);
    let tiles = tiles_in_rect(width, height, saved);
    assert!(tiles.contains(&TileCoord { x: 0, y: 0 }));
}

fn placed(tools: &[Tool]) -> Vec<Placement> {
    tools
        .iter()
        .map(|tool| {
            let profile = tool.profile();
            let mut record: super::DabRecord = bytemuck::Zeroable::zeroed();
            record.rest_follow = profile.rest_follow;
            Placement {
                record,
                canvas_rect: [0.0; 4],
                canvas_groups: [1; 2],
                tip_groups: [1; 2],
                spread_groups: [1; 2],
                profile,
            }
        })
        .collect()
}

fn first_rests(first_rest: &mut FirstRest, tools: &[Tool]) -> Vec<u32> {
    let mut placements = placed(tools);
    first_rest.take(&mut placements);
    placements
        .iter()
        .map(|placement| placement.record.first_rest)
        .collect()
}

#[test]
fn a_new_engine_marks_the_first_resting_dab_it_places() {
    for tool in RESTING_TOOLS {
        assert_eq!(first_rests(&mut FirstRest::new(), &[tool; 3]), [1, 0, 0]);
    }
}

#[test]
fn only_the_first_resting_dab_of_each_stroke_is_its_first_rest() {
    for tool in RESTING_TOOLS {
        let mut first_rest = FirstRest::new();
        first_rest.begin_stroke();
        assert_eq!(
            first_rests(&mut first_rest, &[tool; 3]),
            [1, 0, 0],
            "{tool:?}"
        );
        assert_eq!(first_rests(&mut first_rest, &[tool; 2]), [0, 0], "{tool:?}");
        first_rest.begin_stroke();
        assert_eq!(first_rests(&mut first_rest, &[tool; 2]), [1, 0], "{tool:?}");
    }
}

#[test]
fn the_first_rest_keeps_the_tool_s_own_follow() {
    for tool in RESTING_TOOLS {
        let mut placements = placed(&[tool]);
        FirstRest::new().take(&mut placements);
        assert_eq!(placements[0].record.rest_follow, tool.profile().rest_follow);
    }
}

#[test]
fn wiping_dabs_leave_the_first_rest_to_the_next_resting_dab() {
    let mut first_rest = FirstRest::new();
    assert_eq!(first_rests(&mut first_rest, &[Tool::Rag; 2]), [0, 0]);
    assert_eq!(
        first_rests(&mut first_rest, &[Tool::Rag, Tool::Brush, Tool::Brush]),
        [0, 1, 0]
    );
    assert_eq!(first_rests(&mut first_rest, &[Tool::Brush]), [0]);
}
