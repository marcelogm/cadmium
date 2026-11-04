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
fn one_pan_ending_leaves_the_other_panning() {
    let pans = PanFlags::default();
    pans.set(PanGesture::Primary, true);
    pans.set(PanGesture::Middle, true);
    pans.set(PanGesture::Middle, false);
    assert!(pans.any());
    assert!(pans.is_panning(PanGesture::Primary));
    assert!(!pans.is_panning(PanGesture::Middle));
    pans.set(PanGesture::Primary, false);
    assert!(!pans.any());
}

#[test]
fn another_device_leaving_does_not_end_the_stroke() {
    let (pen, mouse) = ("pen", "mouse");
    assert!(!event_belongs_to_stroke(Some(&pen), Some(&mouse)));
    assert!(event_belongs_to_stroke(Some(&pen), Some(&pen)));
    assert!(event_belongs_to_stroke(Some(&pen), None));
    assert!(event_belongs_to_stroke(None, Some(&mouse)));
}
