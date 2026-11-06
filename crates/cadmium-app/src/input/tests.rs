use super::nav_drag::NavMode;
use super::*;

#[test]
fn side_buttons_do_nothing_while_the_pen_is_painting() {
    assert!(!side_button_may_act(true));
    assert!(side_button_may_act(false));
}

#[test]
fn pen_down_during_a_pan_does_not_paint() {
    assert!(!stroke_may_begin(true));
    assert!(stroke_may_begin(false));
}

#[test]
fn one_drag_ending_leaves_the_other_navigating() {
    let drags = NavDrags::default();
    drags.set(
        NavButton::Primary,
        Some(NavDrag::new(NavMode::Pan, (0.0, 0.0))),
    );
    drags.set(
        NavButton::Middle,
        Some(NavDrag::new(NavMode::Zoom, (0.0, 0.0))),
    );
    drags.set(NavButton::Middle, None);
    assert!(drags.any());
    drags.set(NavButton::Primary, None);
    assert!(!drags.any());
}

#[test]
fn another_device_leaving_does_not_end_the_stroke() {
    let (pen, mouse) = ("pen", "mouse");
    assert!(!event_belongs_to_stroke(Some(&pen), Some(&mouse)));
    assert!(event_belongs_to_stroke(Some(&pen), Some(&pen)));
    assert!(event_belongs_to_stroke(Some(&pen), None));
    assert!(event_belongs_to_stroke(None, Some(&mouse)));
}
