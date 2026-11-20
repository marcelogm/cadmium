//! Measures what one frame of painting costs, scenario by scenario.
//!
//! cargo run --release -p cadmium-engine --example bristle_cost
//!
//! Each scenario paints on a fresh 2048×1536 canvas, the size the app starts with, and
//! drives it the way the app does: a 2000 px S-curve drawn at 800 px/s, sampled at
//! 120 samples/s and framed at 60 frames/s. It prints the mean and 95th percentile of each
//! frame's GPU time, from timestamp queries around the frame when the adapter has them,
//! and of its wall-clock time from encoding until the GPU is done.
//!
//! The long-stroke scenarios paint the S-curve four times back and forth in one stroke and
//! also report each quarter of it, so a cost that grows along a stroke shows up.
//! `CADMIUM_SCENARIO=<text>` measures only the scenarios whose name contains the text.
//! `CADMIUM_PEN_SPEED=<px/s>` draws the strokes at that speed instead of 800 px/s.
//! `CADMIUM_PACED=1` paces the dabs over frames the way the app does, and also prints how
//! many frames a stroke kept painting after the pen lifted (with one repeat only).
//! `CADMIUM_BENCH_STAGES=1` also prints each dab stage's GPU time per dab. It splits the
//! passes, which changes the frame times, so do not check the gate on such a run.
//!
//! `CADMIUM_BENCH_REPEATS=<n>` measures each scenario n times (default 1), on a fresh canvas
//! each time, and reports and records the median of the n means and of the n p95s. The
//! quarters of the long strokes are only printed when n is 1. When n is above 1 the repeats
//! are interleaved, each pass going through all the scenarios before the next begins, after
//! one discarded warm-up pass, so that a slow spell of the GPU is spread over the scenarios;
//! a single repeat has no warm-up. A value that is not a positive integer is an error.
//!
//! `CADMIUM_BENCH_OUT=<file>` also writes each scenario's results to the file as
//! tab-separated values, with the adapter and n. `CADMIUM_BENCH_BASELINE=<file>` compares
//! the results with a file written that way, and a check without `CADMIUM_BENCH_REPEATS`
//! uses the n of its baseline, so that it is measured the way the baseline was.
//!
//! The check exits with status 1 when a scenario got slower by more than 10% and by more
//! than 0.05 ms, in its mean or its p95; the 0.05 ms floor keeps jitter from failing the
//! scenarios that cost almost nothing. It also exits with status 1 when the baseline is
//! unreadable or empty, or has none of the scenarios measured. It warns, and carries on,
//! when the baseline was recorded on another adapter or with another n, or when the run is
//! filtered by `CADMIUM_SCENARIO`, which warms the GPU less than a full run: check full runs.

mod bench;
mod gate;
mod record;
mod scenarios;
mod stages;
mod summary;
mod timing;

use std::path::Path;

use cadmium_engine::canvas::{CanvasSpec, Primer, Weave};
use cadmium_engine::frame_budget::DabBudget;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;

use bench::{Bench, STROKE_PRESSURE, long_stroke, s_curve};
use scenarios::{Ground, Motion, SCENARIOS, Scenario};
use stages::StageTally;
use summary::{Measured, measured, median_of, print_header, print_row, report_quarters};
use timing::{FrameCost, Timestamps, timestamps};

const CANVAS_WIDTH_PX: u32 = 2048;
const CANVAS_HEIGHT_PX: u32 = 1536;
const SCENARIO_FILTER_VARIABLE: &str = "CADMIUM_SCENARIO";
const PACED_VARIABLE: &str = "CADMIUM_PACED";
const PIGMENTS: [&str; 3] = ["cadmium_red", "ultramarine_blue", "titanium_white"];

fn measure(
    gpu: &GpuContext,
    timestamps: Option<&Timestamps>,
    scenario: &Scenario,
    report_lift: bool,
) -> Vec<FrameCost> {
    let spec = CanvasSpec {
        width: CANVAS_WIDTH_PX,
        height: CANVAS_HEIGHT_PX,
        primer: Primer::WhiteGesso,
        weave: Weave::Linen,
        pigment_slots: PIGMENTS.iter().map(|id| id.to_string()).collect(),
        margin_px: 0,
    };
    let mut bench = Bench {
        gpu,
        session: PaintSession::new(gpu, spec).expect("the measuring canvas is valid"),
        timestamps,
        present_target: None,
        frame_index: 0,
        pacing: std::env::var_os(PACED_VARIABLE).map(|_| DabBudget::default()),
        frames_after_lift: 0,
        stages: None,
    };
    if scenario.ground == Ground::Impasto {
        bench.lay_impasto();
    }
    bench.take_up(scenario);
    bench.stages = start_stage_timing(&mut bench);
    let costs = match scenario.motion {
        Motion::Stroke => bench.stroke(&s_curve(STROKE_PRESSURE)),
        Motion::LongStroke => bench.stroke(&long_stroke(STROKE_PRESSURE)),
        Motion::Settle => bench.settle(),
        Motion::Shade => bench.shade(),
        Motion::Present => bench.present(),
    };
    report(&bench, scenario, report_lift);
    costs
}

fn report(bench: &Bench, scenario: &Scenario, report_lift: bool) {
    if report_lift && bench.pacing.is_some() && bench.frames_after_lift > 0 {
        println!(
            "  painting {} frames after the pen lifted",
            bench.frames_after_lift
        );
    }
    if let Some(tally) = &bench.stages {
        println!("{} stages:", scenario.name);
        tally.print();
    }
}

fn start_stage_timing(bench: &mut Bench) -> Option<StageTally> {
    (stages::wanted() && bench.session.engine.time_dab_stages(bench.gpu)).then(StageTally::default)
}

fn selected(scenario: &Scenario, filter: Option<&str>) -> bool {
    filter.is_none_or(|text| scenario.name.contains(text))
}

fn measure_pass(
    gpu: &GpuContext,
    timestamps: Option<&Timestamps>,
    scenarios: &[&Scenario],
    report_lift: bool,
) -> Vec<Vec<FrameCost>> {
    scenarios
        .iter()
        .map(|scenario| measure(gpu, timestamps, scenario, report_lift))
        .collect()
}

fn measure_all(
    gpu: &GpuContext,
    timestamps: Option<&Timestamps>,
    filter: Option<&str>,
    repeats: usize,
) -> Vec<Measured> {
    print_header();
    let scenarios: Vec<&Scenario> = SCENARIOS
        .iter()
        .filter(|scenario| selected(scenario, filter))
        .collect();
    if repeats > 1 {
        measure_pass(gpu, timestamps, &scenarios, false);
    }
    let passes: Vec<_> = (0..repeats)
        .map(|_| measure_pass(gpu, timestamps, &scenarios, repeats == 1))
        .collect();
    let mut rows = Vec::new();
    for (index, scenario) in scenarios.iter().enumerate() {
        let each: Vec<Measured> = passes
            .iter()
            .map(|pass| measured(scenario.name, &pass[index]))
            .collect();
        let row = median_of(&each);
        print_row(&row);
        if scenario.motion == Motion::LongStroke && repeats == 1 {
            report_quarters(&passes[0][index]);
        }
        rows.push(row);
    }
    rows
}

fn describe(gpu: &GpuContext, has_timestamps: bool) {
    let info = gpu.adapter.get_info();
    println!(
        "adapter: {} ({:?}, {})",
        info.name, info.device_type, info.driver_info
    );
    println!(
        "GPU time: {}",
        if has_timestamps {
            "timestamp queries"
        } else {
            "unavailable, wall clock only"
        }
    );
}

fn run() -> Result<(), Vec<String>> {
    let asked = gate::asked_repeats().map_err(|message| vec![message])?;
    let baseline = std::env::var_os(record::BASELINE_VARIABLE)
        .map(|path| gate::load(Path::new(&path)))
        .transpose()?;
    let filter = std::env::var(SCENARIO_FILTER_VARIABLE).ok();
    let gpu = GpuContext::new().expect("a Vulkan adapter to measure on");
    let adapter = gpu.adapter.get_info().name;
    let timestamps = timestamps(&gpu);
    describe(&gpu, timestamps.is_some());
    if let Some(baseline) = &baseline {
        for line in gate::warnings(baseline, &adapter, filter.is_some(), asked) {
            println!("{line}");
        }
    }
    let repeats = gate::resolve_repeats(asked, baseline.as_ref());
    let rows = measure_all(&gpu, timestamps.as_ref(), filter.as_deref(), repeats);
    if let Some(path) = std::env::var_os(record::OUT_VARIABLE) {
        record::write(Path::new(&path), &adapter, repeats, &rows)
            .expect("the results file is writable");
    }
    if let Some(baseline) = baseline {
        println!("{}", gate::verdict(&baseline, &rows)?);
    }
    Ok(())
}

fn main() {
    if let Err(lines) = run() {
        for line in lines {
            println!("{line}");
        }
        std::process::exit(1);
    }
}
