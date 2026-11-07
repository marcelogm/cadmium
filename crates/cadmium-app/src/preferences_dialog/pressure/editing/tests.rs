use super::*;

const RADIUS: f32 = 0.05;

fn curve(points: &[(f32, f32)]) -> PressureCurve {
    PressureCurve::from_points(points).unwrap()
}

fn three() -> PressureCurve {
    curve(&[(0.0, 0.0), (0.5, 0.7), (1.0, 1.0)])
}

#[test]
fn a_press_on_a_point_grabs_it() {
    assert_eq!(grab(&three(), (0.52, 0.68), RADIUS), Some((three(), 1)));
}

#[test]
fn a_press_between_two_close_points_grabs_the_nearer() {
    let close = curve(&[(0.0, 0.0), (0.5, 0.5), (0.55, 0.55), (1.0, 1.0)]);
    assert_eq!(grab(&close, (0.54, 0.54), RADIUS).map(|(_, i)| i), Some(2));
}

#[test]
fn a_press_on_empty_space_adds_a_point_there() {
    let (added, index) = grab(&three(), (0.25, 0.1), RADIUS).unwrap();
    assert_eq!(index, 1);
    assert_eq!(
        added.points(),
        &[(0.0, 0.0), (0.25, 0.1), (0.5, 0.7), (1.0, 1.0)]
    );
}

#[test]
fn no_point_is_added_too_close_to_another_input() {
    assert_eq!(grab(&three(), (0.51, 0.1), RADIUS), None);
}

#[test]
fn no_point_is_added_past_the_most() {
    let full: Vec<(f32, f32)> = (0..MAX_POINTS - 1)
        .map(|i| (i as f32 / 10.0, 0.0))
        .chain([(1.0, 1.0)])
        .collect();
    assert_eq!(grab(&curve(&full), (0.85, 0.2), RADIUS), None);
}

#[test]
fn an_inner_point_follows_the_pen() {
    let moved = drag(&three(), 1, (0.4, 0.2));
    assert_eq!(moved.points()[1], (0.4, 0.2));
}

#[test]
fn an_inner_point_stops_short_of_its_neighbours() {
    let moved = drag(&three(), 1, (0.999, 0.5));
    let x = moved.points()[1].0;
    assert!(x <= 1.0 - MIN_GAP + 1e-5 && x > 0.9, "{x}");
    let moved = drag(&three(), 1, (0.0, 0.5));
    let x = moved.points()[1].0;
    assert!((MIN_GAP - 1e-5..0.1).contains(&x), "{x}");
}

#[test]
fn the_end_points_only_move_up_and_down() {
    let moved = drag(&three(), 0, (0.3, 0.2));
    assert_eq!(moved.points()[0], (0.0, 0.2));
    let moved = drag(&three(), 2, (0.6, 0.8));
    assert_eq!(moved.points()[2], (1.0, 0.8));
}

#[test]
fn removing_an_inner_point_drops_it() {
    assert_eq!(remove(&three(), 1), PressureCurve::linear());
}

#[test]
fn the_end_points_cannot_be_removed() {
    assert_eq!(remove(&three(), 0), three());
    assert_eq!(remove(&three(), 2), three());
}

#[test]
fn an_index_past_the_points_changes_nothing() {
    assert_eq!(drag(&three(), 9, (0.5, 0.5)), three());
    assert_eq!(remove(&three(), 9), three());
}
