//! Frame costs summarised: mean and 95th percentile per scenario, the median of repeats,
//! and the table rows that print them.

use crate::timing::FrameCost;

const PERCENTILE: f64 = 0.95;

fn mean_and_percentile(mut values: Vec<f64>) -> (f64, f64) {
    values.sort_by(f64::total_cmp);
    let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
    let rank = ((values.len() as f64 * PERCENTILE).ceil() as usize).saturating_sub(1);
    (mean, values.get(rank).copied().unwrap_or(0.0))
}

/// One scenario's frame costs, summarised: mean and 95th percentile, in ms.
pub(crate) struct Measured {
    pub(crate) name: String,
    pub(crate) frames: usize,
    pub(crate) gpu: Option<(f64, f64)>,
    pub(crate) wall: (f64, f64),
}

pub(crate) fn measured(name: &str, costs: &[FrameCost]) -> Measured {
    let gpu: Option<Vec<f64>> = costs.iter().map(|cost| cost.gpu_ms).collect();
    Measured {
        name: name.to_string(),
        frames: costs.len(),
        gpu: gpu.map(mean_and_percentile),
        wall: mean_and_percentile(costs.iter().map(|cost| cost.wall_ms).collect()),
    }
}

pub(crate) fn print_header() {
    println!(
        "{:<36} {:>6} {:>9} {:>9} {:>9} {:>9}",
        "scenario (ms per frame)", "frames", "gpu mean", "gpu p95", "wall mean", "wall p95"
    );
}

pub(crate) fn print_row(row: &Measured) {
    let gpu = row.gpu.map_or_else(
        || format!("{:>9} {:>9}", "-", "-"),
        |(mean, p95)| format!("{mean:>9.2} {p95:>9.2}"),
    );
    let (wall_mean, wall_p95) = row.wall;
    println!(
        "{:<36} {:>6} {gpu} {wall_mean:>9.2} {wall_p95:>9.2}",
        row.name, row.frames
    );
}

pub(crate) fn report(name: &str, costs: &[FrameCost]) {
    print_row(&measured(name, costs));
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        (values[middle - 1] + values[middle]) / 2.0
    }
}

fn median_pair(pairs: Vec<(f64, f64)>) -> (f64, f64) {
    (
        median(pairs.iter().map(|pair| pair.0).collect()),
        median(pairs.iter().map(|pair| pair.1).collect()),
    )
}

/// The repeats of one scenario as a single row: the median of each mean and of each p95,
/// and the frame count of the first repeat. It has GPU time only when every repeat does.
pub(crate) fn median_of(runs: &[Measured]) -> Measured {
    let gpu: Option<Vec<(f64, f64)>> = runs.iter().map(|run| run.gpu).collect();
    Measured {
        name: runs[0].name.clone(),
        frames: runs[0].frames,
        gpu: gpu.map(median_pair),
        wall: median_pair(runs.iter().map(|run| run.wall).collect()),
    }
}

const QUARTERS: usize = 4;
const QUARTER_NAMES: [&str; QUARTERS] = [
    "  first quarter",
    "  second quarter",
    "  third quarter",
    "  last quarter",
];

pub(crate) fn report_quarters(costs: &[FrameCost]) {
    let quarter = costs.len().div_ceil(QUARTERS).max(1);
    for (name, chunk) in QUARTER_NAMES.iter().zip(costs.chunks(quarter)) {
        report(name, chunk);
    }
}
