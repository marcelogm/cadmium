use super::*;

fn dab(pressure: f32, altitude: f32) -> Dab {
    Dab {
        x: 10.0,
        y: 20.0,
        radius: 8.0,
        pressure,
        altitude,
        azimuth: 0.0,
        direction: 0.0,
        speed: 0.0,
        time_ms: 0.0,
    }
}

fn gauge() -> HandleGauge {
    HandleGauge {
        reach_px: 40.0,
        ferrule_radius_px: 10.0,
        press_depth: 0.5,
    }
}

#[test]
fn the_clock_keeps_running_between_strokes() {
    let mut clock = HandleClock::default();
    assert_eq!(clock.elapsed_seconds(120.0), 0.0);
    assert_eq!(clock.elapsed_seconds(130.0), 0.010);
    assert_eq!(
        clock.elapsed_seconds(900.0),
        (MAX_STEP_MS / MILLISECONDS_PER_SECOND) as f32
    );
}

#[test]
fn elapsed_time_is_clamped_to_one_step_and_never_runs_backwards() {
    let mut clock = HandleClock::default();
    clock.elapsed_seconds(30.0);
    assert_eq!(clock.elapsed_seconds(10.0), 0.0);
    assert_eq!(
        clock.elapsed_seconds(5000.0),
        (MAX_STEP_MS / MILLISECONDS_PER_SECOND) as f32
    );
    assert_eq!(clock.elapsed_seconds(f32::NAN), 0.0);
}

#[test]
fn pressing_lowers_the_ferrule_by_the_press_depth() {
    let upright = std::f32::consts::FRAC_PI_2;
    let lifted = gauge().lifted(&dab(1.0, upright)).ferrule_px;
    for (got, want) in lifted.into_iter().zip([10.0, 20.0, 40.0]) {
        assert!((got - want).abs() < 1e-4, "{lifted:?}");
    }
    assert_eq!(gauge().pose(&dab(1.0, upright)).ferrule_px[2], 20.0);
}

#[test]
fn a_leaning_handle_keeps_its_ferrule_above_the_canvas() {
    let leaning = 0.1;
    let pose = gauge().pose(&dab(1.0, leaning));
    assert!(pose.ferrule_px[2] >= 10.0 * leaning.cos() - 1e-4);
    let axis = pose.axis;
    assert!((axis[0].hypot(axis[1]).hypot(axis[2]) - 1.0).abs() < 1e-6);
}

#[test]
fn substeps_follow_the_travel_and_stay_bounded() {
    assert_eq!(substeps(0.0, 2.0), 1);
    assert_eq!(substeps(2.5, 2.0), 3);
    assert_eq!(substeps(1e6, 2.0), MAX_SUBSTEPS);
    assert_eq!(substeps(f32::INFINITY, 2.0), MAX_SUBSTEPS);
}

#[test]
fn pressing_offsets_the_ferrule_by_the_press_depth_of_the_whole_length() {
    assert_eq!(gauge().offset_px(0.0), 0.0);
    assert_eq!(gauge().offset_px(1.0), -0.5 * 40.0);
    assert_eq!(gauge().offset_px(2.0), -0.5 * 40.0);
    let altitude = std::f32::consts::FRAC_PI_3;
    assert!((gauge().hanging_px(altitude) - 40.0 * altitude.sin()).abs() < 1e-4);
}

#[test]
fn a_leaning_brush_hangs_its_tips_onto_the_nib() {
    let altitude = std::f32::consts::FRAC_PI_4;
    let pose = gauge().lifted(&dab(0.0, altitude));
    let tip: Vec<f32> = (0..3)
        .map(|axis| pose.ferrule_px[axis] - pose.axis[axis] * 40.0)
        .collect();
    assert!((tip[0] - 10.0).abs() < 1e-3, "{tip:?}");
    assert!((tip[1] - 20.0).abs() < 1e-3, "{tip:?}");
    assert!(tip[2].abs() < 1e-3, "{tip:?}");
}
