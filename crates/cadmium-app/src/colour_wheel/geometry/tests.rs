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

#[test]
fn a_press_on_a_rim_point_grabs_it_and_one_between_two_grabs_nothing() {
    let (x, y) = rim_centre(1, 4, 400.0);
    assert_eq!(rim_hit(x + 5.0, y - 5.0, 4, 400.0), Some(1));
    assert_eq!(rim_hit(200.0, 200.0, 4, 400.0), None);
    assert_eq!(rim_hit(x, y, 0, 400.0), None);
}

#[test]
fn only_well_past_the_edge_counts_as_dragged_out() {
    assert!(!is_outside(200.0, 0.0, 400.0));
    assert!(!is_outside(200.0, -REMOVE_REACH_PX + 1.0, 400.0));
    assert!(is_outside(200.0, -REMOVE_REACH_PX - 1.0, 400.0));
}

#[test]
fn a_turn_is_read_outside_the_disc_too() {
    assert!((turn_at(500.0, 200.0, 400.0) - 0.25).abs() < 1e-4);
    assert!((turn_at(200.0, 500.0, 400.0) - 0.5).abs() < 1e-4);
}

#[test]
fn a_press_between_two_crowded_rim_points_grabs_the_nearer() {
    let (first, second) = (rim_centre(0, 64, 400.0), rim_centre(1, 64, 400.0));
    let between = |share: f64| {
        (
            first.0 + (second.0 - first.0) * share,
            first.1 + (second.1 - first.1) * share,
        )
    };
    let (x, y) = between(0.4);
    assert_eq!(rim_hit(x, y, 64, 400.0), Some(0));
    let (x, y) = between(0.6);
    assert_eq!(rim_hit(x, y, 64, 400.0), Some(1));
}
