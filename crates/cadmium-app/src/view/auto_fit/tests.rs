use super::*;

fn calm() -> bool {
    false
}

fn stroking() -> bool {
    true
}

#[test]
fn a_new_size_fits_the_canvas() {
    assert!(AutoFit::default().on_size_change(true, calm));
}

#[test]
fn a_widget_without_a_size_does_not_fit() {
    assert!(!AutoFit::default().on_size_change(false, calm));
}

#[test]
fn a_view_the_painter_moved_is_left_alone() {
    let mut fit = AutoFit::default();
    fit.moved_by_user();
    assert!(!fit.on_size_change(true, calm));
}

#[test]
fn a_fit_waits_for_the_stroke_then_happens_on_a_frame() {
    let mut fit = AutoFit::default();
    assert!(!fit.on_size_change(true, stroking));
    assert!(!fit.on_frame(stroking));
    assert!(fit.on_frame(calm));
    assert!(!fit.on_frame(calm), "the held-back fit happens once");
}

#[test]
fn frames_do_not_fit_when_nothing_was_held_back() {
    assert!(!AutoFit::default().on_frame(calm));
}

#[test]
fn moving_the_view_pauses_a_held_back_fit_until_the_painter_fits_by_hand() {
    let mut fit = AutoFit::default();
    fit.on_size_change(true, stroking);
    fit.moved_by_user();
    assert!(!fit.on_frame(calm));
    fit.fitted();
    assert!(fit.on_frame(calm), "the held-back fit is still owed");
    assert!(!fit.on_frame(calm));
}

#[test]
fn fitting_by_hand_lets_resizes_fit_again() {
    let mut fit = AutoFit::default();
    fit.moved_by_user();
    fit.fitted();
    assert!(fit.on_size_change(true, calm));
}

#[test]
fn the_stroke_is_only_asked_about_when_a_fit_is_due() {
    let mut fit = AutoFit::default();
    fit.moved_by_user();
    assert!(!fit.on_size_change(true, || panic!("not asked")));
    assert!(!AutoFit::default().on_size_change(false, || panic!("not asked")));
    assert!(!AutoFit::default().on_frame(|| panic!("not asked")));
}

#[test]
fn a_rule_kept_in_a_cell_is_changed_in_place() {
    let cell = std::cell::Cell::new(AutoFit::default());
    update_auto_fit(&cell, AutoFit::moved_by_user);
    assert!(!update_auto_fit(&cell, |fit| fit.on_size_change(true, calm)));
    update_auto_fit(&cell, AutoFit::fitted);
    assert!(update_auto_fit(&cell, |fit| fit.on_size_change(true, calm)));
}
