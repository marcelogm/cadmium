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
//! many frames a stroke kept painting after the pen lifted.
//! `CADMIUM_BENCH_REPEATS=<n>` measures each scenario n times (default 1), on a fresh canvas
//! each time, and reports and records the median of the n means and of the n p95s; the
//! quarters of the long strokes are only printed when n is 1. When n is above 1 the repeats
//! are interleaved, each pass going through all the scenarios before the next begins, after
//! one discarded warm-up pass, so that a slow spell of the GPU is spread over the scenarios.
//! The results file records n, and a check without `CADMIUM_BENCH_REPEATS` uses the n of its
//! baseline, so that it is measured the way the baseline was. Check full runs only: a run
//! filtered by `CADMIUM_SCENARIO` warms the GPU less and warns when it has a baseline.
//! `CADMIUM_BENCH_OUT=<file>` also writes each scenario's results to the file as
//! tab-separated values; `CADMIUM_BENCH_BASELINE=<file>` compares them with a file written
//! that way and exits with status 1 when a scenario got slower by more than 10% and by more
//! than 0.05 ms, in its mean or its p95. The 0.05 ms floor keeps jitter from failing the
//! scenarios that cost almost nothing.

mod bench;
mod record;
mod scenarios;
mod timing;

use std::path::Path;

use cadmium_engine::canvas::{CanvasSpec, Primer, Weave};
use cadmium_engine::frame_budget::DabBudget;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;

use bench::{Bench, STROKE_PRESSURE, long_stroke, s_curve};
use scenarios::{Ground, Motion, SCENARIOS, Scenario};
use timing::{
    FrameCost, Measured, Timestamps, measured, median_of, print_row, report_quarters, timestamps,
};

const CANVAS_WIDTH_PX: u32 = 2048;
const CANVAS_HEIGHT_PX: u32 = 1536;
const SCENARIO_FILTER_VARIABLE: &str = "CADMIUM_SCENARIO";
const REPEATS_VARIABLE: &str = "CADMIUM_BENCH_REPEATS";
const PACED_VARIABLE: &str = "CADMIUM_PACED";
const PIGMENTS: [&str; 3] = ["cadmium_red", "ultramarine_blue", "titanium_white"];

fn measure(
    gpu: &GpuContext,
    timestamps: Option<&Timestamps>,
    scenario: &Scenario,
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
    };
    if scenario.ground == Ground::Impasto {
        bench.lay_impasto();
    }
    bench.take_up(scenario);
    let costs = match scenario.motion {
        Motion::Stroke => bench.stroke(&s_curve(STROKE_PRESSURE)),
        Motion::LongStroke => bench.stroke(&long_stroke(STROKE_PRESSURE)),
        Motion::Settle => bench.settle(),
        Motion::Shade => bench.shade(),
        Motion::Present => bench.present(),
    };
    if bench.pacing.is_some() && bench.frames_after_lift > 0 {
        println!(
            "  painting {} frames after the pen lifted",
            bench.frames_after_lift
        );
    }
    costs
}

fn selected(scenario: &Scenario) -> bool {
    match std::env::var(SCENARIO_FILTER_VARIABLE) {
        Ok(filter) => scenario.name.contains(filter.as_str()),
        Err(_) => true,
    }
}

/// The repeats asked for, else those the baseline was recorded with, else 1.
fn repeats(baseline: Option<&record::Baseline>) -> usize {
    let asked: Option<usize> = std::env::var(REPEATS_VARIABLE)
        .ok()
        .and_then(|text| text.parse().ok())
        .filter(|&count| count > 0);
    let recorded = baseline.and_then(|baseline| baseline.repeats);
    if let (Some(asked), Some(recorded)) = (asked, recorded)
        && asked != recorded
    {
        println!("warning: baseline recorded with {recorded} repeats, measuring with {asked}");
    }
    asked.or(recorded).unwrap_or(1)
}

fn measure_pass(
    gpu: &GpuContext,
    timestamps: Option<&Timestamps>,
    scenarios: &[&Scenario],
) -> Vec<Vec<FrameCost>> {
    scenarios
        .iter()
        .map(|scenario| measure(gpu, timestamps, scenario))
        .collect()
}

fn measure_all(gpu: &GpuContext, timestamps: Option<&Timestamps>, repeats: usize) -> Vec<Measured> {
    println!(
        "{:<36} {:>6} {:>9} {:>9} {:>9} {:>9}",
        "scenario (ms per frame)", "frames", "gpu mean", "gpu p95", "wall mean", "wall p95"
    );
    let scenarios: Vec<&Scenario> = SCENARIOS
        .iter()
        .filter(|scenario| selected(scenario))
        .collect();
    if repeats > 1 {
        measure_pass(gpu, timestamps, &scenarios);
    }
    let passes: Vec<_> = (0..repeats)
        .map(|_| measure_pass(gpu, timestamps, &scenarios))
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

fn fail(message: &str) -> ! {
    println!("{message}");
    std::process::exit(1);
}

fn load_baseline(path: &Path) -> record::Baseline {
    let baseline = record::read(path).expect("the baseline file is readable");
    if baseline.rows.is_empty() {
        fail(&format!(
            "the baseline at {} holds no results",
            path.display()
        ));
    }
    baseline
}

fn warn_if_filtered() {
    if std::env::var_os(SCENARIO_FILTER_VARIABLE).is_some() {
        println!(
            "warning: a filtered run warms the GPU less than the full run the baseline was \
             recorded with; compare full runs only"
        );
    }
}

fn check_against(baseline: &record::Baseline, adapter: &str, rows: &[Measured]) {
    if let Some(recorded) = baseline.adapter.as_deref().filter(|name| *name != adapter) {
        println!("warning: baseline recorded on {recorded}, measuring on {adapter}");
    }
    let compared = record::compared(&baseline.rows, rows);
    println!(
        "compared {compared} of {} baseline scenarios",
        baseline.rows.len()
    );
    if compared == 0 {
        fail("no scenario of this run is in the baseline");
    }
    let slower = record::over_budget(&baseline.rows, rows);
    if slower.is_empty() {
        println!(
            "within {:.0}% of the baseline",
            (record::ALLOWED_SLOWDOWN - 1.0) * 100.0
        );
        return;
    }
    for line in &slower {
        println!("over budget: {line}");
    }
    std::process::exit(1);
}

fn main() {
    let gpu = GpuContext::new().expect("a Vulkan adapter to measure on");
    let info = gpu.adapter.get_info();
    let timestamps = timestamps(&gpu);
    println!(
        "adapter: {} ({:?}, {})",
        info.name, info.device_type, info.driver_info
    );
    println!(
        "GPU time: {}",
        if timestamps.is_some() {
            "timestamp queries"
        } else {
            "unavailable, wall clock only"
        }
    );
    let baseline =
        std::env::var_os(record::BASELINE_VARIABLE).map(|path| load_baseline(Path::new(&path)));
    if baseline.is_some() {
        warn_if_filtered();
    }
    let repeats = repeats(baseline.as_ref());
    let rows = measure_all(&gpu, timestamps.as_ref(), repeats);
    if let Some(path) = std::env::var_os(record::OUT_VARIABLE) {
        record::write(Path::new(&path), &info.name, repeats, &rows)
            .expect("the results file is writable");
    }
    if let Some(baseline) = baseline {
        check_against(&baseline, &info.name, &rows);
    }
}
