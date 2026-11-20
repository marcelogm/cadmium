use std::collections::BTreeMap;

use cadmium_engine::engine::DabStageTime;

const STAGES_VARIABLE: &str = "CADMIUM_BENCH_STAGES";
const MICROSECONDS_PER_MILLISECOND: f64 = 1000.0;

pub(crate) fn wanted() -> bool {
    std::env::var_os(STAGES_VARIABLE).is_some()
}

#[derive(Default)]
pub(crate) struct StageTally {
    totals: BTreeMap<String, (u64, f64)>,
}

impl StageTally {
    pub(crate) fn add(&mut self, times: &[DabStageTime]) {
        for time in times {
            let total = self.totals.entry(time.stage.clone()).or_default();
            total.0 += u64::from(time.dabs);
            total.1 += time.gpu_ms;
        }
    }

    pub(crate) fn per_dab_us(&self) -> Vec<(String, f64, u64)> {
        self.totals
            .iter()
            .filter(|(_, (dabs, _))| *dabs > 0)
            .map(|(stage, (dabs, ms))| {
                (
                    stage.clone(),
                    ms * MICROSECONDS_PER_MILLISECOND / *dabs as f64,
                    *dabs,
                )
            })
            .collect()
    }

    pub(crate) fn print(&self) {
        for (stage, us, dabs) in self.per_dab_us() {
            println!("  {stage:<10} {us:>8.1} µs/dab over {dabs} dabs");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tally_reports_microseconds_per_dab_by_stage() {
        let mut tally = StageTally::default();
        let time = |stage: &str, dabs, gpu_ms| DabStageTime {
            stage: stage.to_string(),
            dabs,
            gpu_ms,
        };
        tally.add(&[time("Bristles", 2, 0.4), time("Exchange", 2, 0.1)]);
        tally.add(&[time("Bristles", 2, 0.2)]);
        let per_dab = tally.per_dab_us();
        assert_eq!(per_dab.len(), 2);
        assert_eq!(per_dab[0].0, "Bristles");
        assert!((per_dab[0].1 - 150.0).abs() < 1e-9);
        assert_eq!(per_dab[0].2, 4);
        assert!((per_dab[1].1 - 50.0).abs() < 1e-9);
    }
}
