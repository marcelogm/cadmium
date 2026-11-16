//! A run's results as tab-separated values, and the check of a run against a recorded one.

use std::fmt::Write as _;
use std::path::Path;

use crate::timing::Measured;

pub(crate) const OUT_VARIABLE: &str = "CADMIUM_BENCH_OUT";
pub(crate) const BASELINE_VARIABLE: &str = "CADMIUM_BENCH_BASELINE";
/// How much slower than the baseline a scenario may get: the budget of the XPBD rewrite (#62).
pub(crate) const ALLOWED_SLOWDOWN: f64 = 1.10;
const HEADER: &str = "scenario\tframes\tgpu_mean_ms\tgpu_p95_ms\twall_mean_ms\twall_p95_ms";
const NO_VALUE: &str = "-";

pub(crate) fn write(path: &Path, adapter: &str, rows: &[Measured]) -> std::io::Result<()> {
    let mut text = format!("# adapter: {adapter}\n{HEADER}\n");
    for row in rows {
        let (gpu_mean, gpu_p95) = row.gpu.map_or_else(
            || (NO_VALUE.to_string(), NO_VALUE.to_string()),
            |(mean, p95)| (format!("{mean:.4}"), format!("{p95:.4}")),
        );
        let (wall_mean, wall_p95) = row.wall;
        let _ = writeln!(
            text,
            "{}\t{}\t{gpu_mean}\t{gpu_p95}\t{wall_mean:.4}\t{wall_p95:.4}",
            row.name, row.frames
        );
    }
    std::fs::write(path, text)
}

pub(crate) fn read(path: &Path) -> std::io::Result<Vec<Measured>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter(|line| !line.starts_with('#') && *line != HEADER)
        .filter_map(parse_row)
        .collect())
}

fn parse_row(line: &str) -> Option<Measured> {
    let fields: Vec<&str> = line.split('\t').collect();
    let [name, frames, gpu_mean, gpu_p95, wall_mean, wall_p95] = fields.as_slice() else {
        return None;
    };
    Some(Measured {
        name: (*name).to_string(),
        frames: frames.parse().ok()?,
        gpu: gpu_mean.parse().ok().zip(gpu_p95.parse().ok()),
        wall: (wall_mean.parse().ok()?, wall_p95.parse().ok()?),
    })
}

/// The scenarios measured in both runs that got slower than `ALLOWED_SLOWDOWN` allows,
/// judged on GPU time when both runs have it and on wall time otherwise.
pub(crate) fn over_budget(baseline: &[Measured], current: &[Measured]) -> Vec<String> {
    current
        .iter()
        .filter_map(|now| {
            let before = baseline.iter().find(|row| row.name == now.name)?;
            slower_than(before, now)
        })
        .collect()
}

fn slower_than(before: &Measured, now: &Measured) -> Option<String> {
    let (then, current, clock) = match (before.gpu, now.gpu) {
        (Some(then), Some(current)) => (then, current, "gpu"),
        _ => (before.wall, now.wall, "wall"),
    };
    let slower = current.0 > then.0 * ALLOWED_SLOWDOWN || current.1 > then.1 * ALLOWED_SLOWDOWN;
    slower.then(|| {
        format!(
            "{}: {clock} mean {:.2} -> {:.2} ms, p95 {:.2} -> {:.2} ms",
            now.name, then.0, current.0, then.1, current.1
        )
    })
}
