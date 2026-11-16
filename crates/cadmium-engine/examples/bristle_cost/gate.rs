//! The policy of checking a run against a recorded one: what to load, what to warn about,
//! and whether the run passes. It prints nothing and never exits; `main` does both.

use std::path::Path;

use crate::record::{self, Baseline};
use crate::summary::Measured;

pub(crate) const REPEATS_VARIABLE: &str = "CADMIUM_BENCH_REPEATS";

/// Reads the baseline, failing when the file is unreadable or holds no results.
pub(crate) fn load(path: &Path) -> Result<Baseline, Vec<String>> {
    let baseline = record::read(path).map_err(|error| {
        vec![format!(
            "cannot read the baseline at {}: {error}",
            path.display()
        )]
    })?;
    if baseline.rows.is_empty() {
        return Err(vec![format!(
            "the baseline at {} holds no results",
            path.display()
        )]);
    }
    Ok(baseline)
}

/// The repeats asked for in the environment, if any; it must be a positive integer.
pub(crate) fn asked_repeats() -> Result<Option<usize>, String> {
    let Ok(text) = std::env::var(REPEATS_VARIABLE) else {
        return Ok(None);
    };
    match text.parse::<usize>() {
        Ok(count) if count > 0 => Ok(Some(count)),
        _ => Err(format!(
            "{REPEATS_VARIABLE} must be a positive integer, not {text:?}"
        )),
    }
}

/// The repeats asked for, else those the baseline was recorded with, else 1.
pub(crate) fn resolve_repeats(asked: Option<usize>, baseline: Option<&Baseline>) -> usize {
    let recorded = baseline.and_then(|baseline| baseline.repeats);
    asked.or(recorded).unwrap_or(1)
}

/// What differs between how the baseline was recorded and how this run is measured.
pub(crate) fn warnings(
    baseline: &Baseline,
    adapter: &str,
    filtered: bool,
    asked_repeats: Option<usize>,
) -> Vec<String> {
    let mut lines = Vec::new();
    if filtered {
        lines.push(
            "warning: a filtered run warms the GPU less than the full run the baseline was \
             recorded with; compare full runs only"
                .to_string(),
        );
    }
    if let Some(recorded) = baseline.adapter.as_deref().filter(|name| *name != adapter) {
        lines.push(format!(
            "warning: baseline recorded on {recorded}, measuring on {adapter}"
        ));
    }
    if let (Some(asked), Some(recorded)) = (asked_repeats, baseline.repeats)
        && asked != recorded
    {
        lines.push(format!(
            "warning: baseline recorded with {recorded} repeats, measuring with {asked}"
        ));
    }
    lines
}

/// The summary of a run within budget, or the lines saying why it is not: nothing compared,
/// or the scenarios over budget.
pub(crate) fn verdict(baseline: &Baseline, rows: &[Measured]) -> Result<String, Vec<String>> {
    let compared = record::compared(&baseline.rows, rows);
    let count = format!(
        "compared {compared} of {} baseline scenarios",
        baseline.rows.len()
    );
    if compared == 0 {
        return Err(vec![
            count,
            "no scenario of this run is in the baseline".to_string(),
        ]);
    }
    let slower = record::over_budget(&baseline.rows, rows);
    if slower.is_empty() {
        let percent = (record::ALLOWED_SLOWDOWN - 1.0) * 100.0;
        return Ok(format!("{count}\nwithin {percent:.0}% of the baseline"));
    }
    let mut lines = vec![count];
    lines.extend(slower.iter().map(|line| format!("over budget: {line}")));
    Err(lines)
}
