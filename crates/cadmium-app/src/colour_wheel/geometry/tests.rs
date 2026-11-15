use super::*;

#[test]
fn the_top_of_the_wheel_is_turn_zero_and_turns_run_clockwise() {
    let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
    let (turn, radius) = turn_and_radius_at(100.0, 0.0, 200.0).unwrap();
    assert!(close(turn, 0.0) && close(radius, 1.0));
    assert!(close(
        turn_and_radius_at(200.0, 100.0, 200.0).unwrap().0,
        0.25
    ));
    assert!(close(
        turn_and_radius_at(100.0, 200.0, 200.0).unwrap().0,
        0.5
    ));
    assert!(close(
        turn_and_radius_at(0.0, 100.0, 200.0).unwrap().0,
        0.75
    ));
}

#[test]
fn the_centre_is_radius_zero_and_outside_the_disc_is_nothing() {
    assert_eq!(turn_and_radius_at(100.0, 100.0, 200.0).unwrap().1, 0.0);
    assert_eq!(turn_and_radius_at(199.0, 199.0, 200.0), None);
}

#[test]
fn rim_points_sit_evenly_round_the_wheel_starting_at_the_top() {
    assert_eq!(rim_centre(0, 4, 200.0), (100.0, RIM_INSET_PX));
    let (x, y) = rim_centre(1, 4, 200.0);
    assert!((x - (200.0 - RIM_INSET_PX)).abs() < 1e-9 && (y - 100.0).abs() < 1e-9);
}

#[test]
fn a_wheel_with_no_size_has_no_point() {
    assert_eq!(turn_and_radius_at(0.0, 0.0, 0.0), None);
    assert_eq!(turn_and_radius_at(3.0, 4.0, 0.0), None);
}

#[test]
fn every_point_of_the_disc_has_a_turn_below_a_whole_turn_or_at_it() {
    for step in 0..360 {
        let angle = f64::from(step).to_radians();
        let (turn, radius) = turn_and_radius_at(
            100.0 + 50.0 * angle.sin(),
            100.0 - 50.0 * angle.cos(),
            200.0,
        )
        .unwrap();
        assert!((turn - step as f32 / 360.0).abs() < 1e-4, "{step}: {turn}");
        assert!((radius - 0.5).abs() < 1e-4);
    }
}

#[test]
fn an_empty_circle_has_a_rim_centre_that_is_a_number() {
    let (x, y) = rim_centre(0, 0, 200.0);
    assert!(x.is_finite() && y.is_finite());
}
