use super::*;

#[test]
fn the_tally_sums_each_stage_over_its_dabs_in_first_seen_order() {
    let passes = [DabStage::Bristles, DabStage::Exchange, DabStage::Bristles];
    let ticks = [10, 30, 30, 35, 40, 70];
    let times = tally(&passes, &ticks, 1.0e6);
    assert_eq!(
        times,
        vec![
            DabStageTime {
                stage: "Bristles".to_string(),
                dabs: 2,
                gpu_ms: 50.0,
            },
            DabStageTime {
                stage: "Exchange".to_string(),
                dabs: 1,
                gpu_ms: 5.0,
            },
        ]
    );
}
