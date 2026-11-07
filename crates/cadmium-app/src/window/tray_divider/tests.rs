use super::*;

#[test]
fn the_divider_leaves_the_tray_its_share_of_each_height() {
    let mut divider = TrayDivider::new(0.25);
    assert_eq!(divider.place(800), Some(600));
    divider.end_place();
    assert_eq!(divider.place(400), Some(300));
    divider.end_place();
    assert_eq!(divider.share(), 0.25);
}

#[test]
fn nothing_is_placed_before_the_paned_has_a_height() {
    let mut divider = TrayDivider::new(0.25);
    assert_eq!(divider.place(0), None);
    divider.follow_move(10, 0);
    assert_eq!(divider.share(), 0.25);
}

#[test]
fn a_move_at_the_laid_out_height_is_the_painters_and_sets_the_share() {
    let mut divider = TrayDivider::new(0.25);
    divider.place(800);
    divider.end_place();
    divider.follow_move(400, 800);
    assert_eq!(divider.share(), 0.5);
}

#[test]
fn the_divider_s_own_placing_does_not_count_as_a_move() {
    let mut divider = TrayDivider::new(0.25);
    divider.place(800);
    // GTK clamps the position it was given and reports the clamped one.
    divider.follow_move(700, 800);
    divider.end_place();
    assert_eq!(divider.share(), 0.25);
}

#[test]
fn a_move_from_a_new_height_waits_for_the_divider_to_be_placed_again() {
    let mut divider = TrayDivider::new(0.25);
    divider.place(800);
    divider.end_place();
    // The window grew: GTK moves the divider itself before it is placed for 1000.
    divider.follow_move(800, 1000);
    assert_eq!(divider.share(), 0.25);
    assert_eq!(divider.place(1000), Some(750));
}

#[test]
fn a_move_is_ignored_before_anything_is_laid_out() {
    let mut divider = TrayDivider::new(0.25);
    divider.follow_move(400, 800);
    assert_eq!(divider.share(), 0.25);
}

#[test]
fn a_move_after_the_layout_is_forgotten_waits_for_the_next_placing() {
    let mut divider = TrayDivider::new(0.25);
    divider.place(800);
    divider.end_place();
    // The tray was just shown: GTK clamps the divider before it is placed again.
    divider.forget_layout();
    divider.follow_move(700, 800);
    assert_eq!(divider.share(), 0.25);
    assert_eq!(divider.place(800), Some(600));
}
