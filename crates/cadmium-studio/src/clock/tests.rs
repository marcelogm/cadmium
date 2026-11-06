use super::quanta_due;
use crate::player::MAX_QUANTA_PER_ADVANCE;

const QUANTUM: f64 = 6.0;
const CAP: u32 = MAX_QUANTA_PER_ADVANCE;

/// Plays `steps` of real time through `quanta_due`, from an empty accumulator.
fn play(steps: &[f64], speed: f64) -> (u32, f64) {
    steps.iter().fold((0, 0.0), |(quanta, acc), dt| {
        let (due, rest) = quanta_due(acc, *dt, speed, QUANTUM, CAP);
        (quanta + due, rest)
    })
}

#[test]
fn any_split_of_the_same_time_gives_the_same_quanta_and_remainder() {
    let total = play(&[40.5], 1.0);
    assert_eq!(total, (6, 4.5));
    for split in [
        &[20.25, 20.25][..],
        &[0.5; 81][..],
        &[1.0, 2.0, 3.0, 4.0, 5.5, 25.0],
        &[6.0, 0.0, 34.5],
    ] {
        assert_eq!(play(split, 1.0), total, "{split:?}");
    }
}

#[test]
fn a_paused_player_runs_no_quanta() {
    assert_eq!(play(&[1000.0, 50.0, 7.0], 0.0), (0, 0.0));
    assert_eq!(quanta_due(5.0, 1000.0, 0.0, QUANTUM, CAP), (0, 5.0));
}

#[test]
fn slow_motion_takes_four_times_the_real_time_for_the_same_quanta() {
    for real_ms in [24.0, 100.0, 96.0, 7.5, 160.0] {
        let slow = play(&[real_ms], 0.25);
        let fast = play(&[real_ms / 4.0], 1.0);
        assert_eq!(slow, fast, "{real_ms}");
    }
    assert_eq!(play(&[24.0; 10], 0.25), play(&[6.0; 10], 1.0));
}

#[test]
fn a_stall_past_the_cap_drops_the_excess_time() {
    let (due, rest) = quanta_due(0.0, 10_000.0, 1.0, QUANTUM, CAP);
    assert_eq!(due, CAP);
    assert!((0.0..QUANTUM).contains(&rest), "{rest}");
    let (due, rest) = quanta_due(3.0, 6.0 * f64::from(CAP), 1.0, QUANTUM, CAP);
    assert_eq!((due, rest), (CAP, 3.0));
    let (due, rest) = quanta_due(3.0, 6.0 * f64::from(CAP) + 3.0, 1.0, QUANTUM, CAP);
    assert_eq!(due, CAP);
    assert!(rest < QUANTUM, "{rest}");
}

#[test]
fn non_finite_time_counts_as_none() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -5.0] {
        assert_eq!(quanta_due(3.0, bad, 1.0, QUANTUM, CAP), (0, 3.0), "{bad}");
        assert_eq!(quanta_due(3.0, 12.0, bad, QUANTUM, CAP), (0, 3.0), "{bad}");
        assert_eq!(quanta_due(bad, 12.0, 1.0, QUANTUM, CAP), (2, 0.0), "{bad}");
    }
    let (due, rest) = quanta_due(0.0, f64::MAX, f64::MAX, QUANTUM, CAP);
    assert_eq!((due, rest), (CAP, 0.0));
}
