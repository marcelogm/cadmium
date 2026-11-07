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

#[test]
fn letting_go_of_the_divider_near_the_bottom_closes_and_keeps_the_last_good_share() {
    let mut divider = TrayDivider::new(0.25);
    divider.place(800);
    divider.end_place();
    divider.grab();
    divider.follow_move(500, 800);
    assert_eq!(divider.share(), 0.375);
    // Dragged on down to leave the tray less than the closing height.
    divider.follow_move(800 - CLOSE_BELOW_PX + 1, 800);
    assert_eq!(divider.share(), 0.375);
    assert!(divider.released());
    divider.follow_move(800 - CLOSE_BELOW_PX + 1, 800);
    assert!(
        !divider.released(),
        "a release closes once, for the drag it ends"
    );
}

#[test]
fn letting_go_above_the_closing_height_keeps_the_tray_open_at_its_new_share() {
    let mut divider = TrayDivider::new(0.25);
    divider.place(800);
    divider.end_place();
    divider.grab();
    divider.follow_move(800 - CLOSE_BELOW_PX + 1, 800);
    // Back up before letting go.
    divider.follow_move(400, 800);
    assert!(!divider.released());
    assert_eq!(divider.share(), 0.5);
}

#[test]
fn only_the_painter_s_drags_close_the_tray() {
    let mut divider = TrayDivider::new(0.25);
    divider.grab();
    divider.place(800);
    divider.follow_move(790, 800);
    divider.end_place();
    // GTK moving the divider at a new height is layout, not a drag.
    divider.follow_move(990, 1000);
    assert!(!divider.released());
    divider.place(1000);
    divider.end_place();
    divider.follow_move(990, 1000);
    divider.forget_layout();
    assert!(!divider.released(), "a tray shown again starts afresh");
}

#[test]
fn a_drag_up_on_the_closed_bar_opens_past_a_small_threshold() {
    assert!(!bar_drag_opens(0.0));
    assert!(!bar_drag_opens(-(OPEN_DRAG_PX - 1.0)));
    assert!(bar_drag_opens(-OPEN_DRAG_PX));
    assert!(bar_drag_opens(-40.0));
    assert!(!bar_drag_opens(40.0), "dragging down does not open");
}

#[test]
fn a_move_without_a_grab_on_the_handle_never_closes() {
    // The keyboard moves the divider with no press on the handle.
    let mut divider = TrayDivider::new(0.25);
    divider.place(800);
    divider.end_place();
    divider.follow_move(790, 800);
    assert!(
        !divider.released(),
        "ending a stroke elsewhere does not close the tray"
    );
    assert_eq!(
        divider.share(),
        0.25,
        "a move that low is not kept as the share"
    );
}

#[test]
fn a_cancelled_grab_does_not_close() {
    let mut divider = TrayDivider::new(0.25);
    divider.place(800);
    divider.end_place();
    divider.grab();
    divider.follow_move(790, 800);
    divider.cancel();
    assert!(!divider.released());
}

#[test]
fn an_open_tray_keeps_at_least_its_own_minimum_height() {
    // A paned 800 tall with a 10 px handle and a tray that needs 180 px.
    assert_eq!(clamp_open_position(700, 0, 800, 10, 180), 610);
    assert_eq!(clamp_open_position(400, 0, 800, 10, 180), 400);
    assert_eq!(clamp_open_position(40, 100, 800, 10, 180), 100);
    // Too short for both: the canvas's minimum wins.
    assert_eq!(clamp_open_position(150, 200, 300, 10, 180), 200);
}
