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

#[test]
fn a_moving_tool_ignores_the_pen() {
    assert_eq!(down_outcome(Press::Pan, false), DownOutcome::Ignore);
    assert_eq!(down_outcome(Press::Pan, true), DownOutcome::Ignore);
}

#[test]
fn the_eyedropper_picks_with_the_tip() {
    assert_eq!(
        down_outcome(Press::PickColour, false),
        DownOutcome::PickColour
    );
}

#[test]
fn the_eraser_end_wipes_even_under_the_eyedropper() {
    assert_eq!(down_outcome(Press::PickColour, true), DownOutcome::Press);
}

#[test]
fn painting_tools_press_into_the_painting() {
    use crate::tools::Tool;
    let presses = [
        Press::Stroke(Tool::Brush),
        Press::Stroke(Tool::Knife),
        Press::Stroke(Tool::Blender),
        Press::Stroke(Tool::Rag),
        Press::Stroke(Tool::Eyedropper),
        Press::LoadFromPile(Tool::Brush),
        Press::LoadFromPile(Tool::Knife),
        Press::Squeeze,
    ];
    for press in presses {
        assert_eq!(down_outcome(press, false), DownOutcome::Press);
        assert_eq!(down_outcome(press, true), DownOutcome::Press);
    }
}
