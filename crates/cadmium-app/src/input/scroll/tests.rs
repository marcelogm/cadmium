use super::*;

fn zoom_factor(action: ScrollAction) -> f32 {
    match action {
        ScrollAction::Zoom { factor } => factor,
        other => panic!("expected a zoom, got {other:?}"),
    }
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "{actual} is not close to {expected}"
    );
}

#[test]
fn a_wheel_notch_zooms_by_one_step() {
    assert_close(
        zoom_factor(scroll_action(ScrollUnit::Wheel, false, 0.0, 1.0)),
        1.0 / 1.1,
    );
    assert_close(
        zoom_factor(scroll_action(ScrollUnit::Wheel, false, 0.0, -2.0)),
        1.21,
    );
}

#[test]
fn a_touchpad_zooms_one_step_per_fifteen_px() {
    assert_close(
        zoom_factor(scroll_action(ScrollUnit::Surface, false, 0.0, 15.0)),
        1.0 / 1.1,
    );
}

#[test]
fn a_touchpad_scroll_of_half_a_step_zooms_by_half_a_step() {
    assert_close(
        zoom_factor(scroll_action(ScrollUnit::Surface, false, 0.0, 7.5)),
        1.1f32.powf(-0.5),
    );
}

#[test]
fn a_sideways_scroll_without_shift_is_passed_on() {
    assert_eq!(
        scroll_action(ScrollUnit::Wheel, false, 3.0, 0.0),
        ScrollAction::Pass
    );
}

#[test]
fn shift_and_the_wheel_pan_sideways() {
    assert_eq!(
        scroll_action(ScrollUnit::Wheel, true, 0.0, 1.0),
        ScrollAction::Pan { dx: -48.0, dy: 0.0 }
    );
}

#[test]
fn shift_and_a_diagonal_wheel_scroll_pan_both_ways() {
    assert_eq!(
        scroll_action(ScrollUnit::Wheel, true, 1.0, 1.0),
        ScrollAction::Pan {
            dx: -48.0,
            dy: -48.0
        }
    );
}

#[test]
fn shift_and_no_movement_pan_nowhere_but_still_take_the_scroll() {
    assert_eq!(
        scroll_action(ScrollUnit::Wheel, true, 0.0, 0.0),
        ScrollAction::Pan { dx: 0.0, dy: 0.0 }
    );
}

#[test]
fn shift_and_a_touchpad_pan_as_far_as_the_fingers_move() {
    assert_eq!(
        scroll_action(ScrollUnit::Surface, true, 4.0, -6.0),
        ScrollAction::Pan { dx: -4.0, dy: 6.0 }
    );
}

#[test]
fn a_pinch_zooms_by_how_much_it_grew_since_last_time() {
    assert_eq!(pinch_factor(1.0, 1.5), Some(1.5));
    assert_eq!(pinch_factor(2.0, 1.0), Some(0.5));
}

#[test]
fn a_pinch_with_no_size_does_nothing() {
    assert_eq!(pinch_factor(0.0, 1.0), None);
    assert_eq!(pinch_factor(1.0, 0.0), None);
}
