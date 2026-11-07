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

mod bench;
mod scenarios;
mod timing;

use cadmium_engine::canvas::{CanvasSpec, Primer, Weave};
use cadmium_engine::frame_budget::DabBudget;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;

use bench::{Bench, STROKE_PRESSURE, long_stroke, s_curve};
use scenarios::{Ground, Motion, SCENARIOS, Scenario};
use timing::{FrameCost, Timestamps, report, report_quarters, timestamps};

const CANVAS_WIDTH_PX: u32 = 2048;
const CANVAS_HEIGHT_PX: u32 = 1536;
const SCENARIO_FILTER_VARIABLE: &str = "CADMIUM_SCENARIO";
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
    println!(
        "{:<36} {:>6} {:>9} {:>9} {:>9} {:>9}",
        "scenario (ms per frame)", "frames", "gpu mean", "gpu p95", "wall mean", "wall p95"
    );
    for scenario in &SCENARIOS {
        if !selected(scenario) {
            continue;
        }
        let costs = measure(&gpu, timestamps.as_ref(), scenario);
        report(scenario.name, &costs);
        if scenario.motion == Motion::LongStroke {
            report_quarters(&costs);
        }
    }
}
