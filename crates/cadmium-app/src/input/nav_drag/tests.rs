use gtk::gdk;

use super::*;
use crate::settings::{PenButtonAction, Settings};

fn press(button: NavButton) -> NavPress {
    NavPress {
        button,
        from_pen: true,
        held: None,
        tool_pans: false,
        stroking: false,
        pen_buttons: PenButtons::Other,
    }
}

#[test]
fn held_keys_pick_the_mode_for_the_primary_contact() {
    let held = |mode| NavPress {
        held: Some(mode),
        ..press(NavButton::Primary)
    };
    let settings = Settings::default();
    assert_eq!(nav_mode(held(NavMode::Pan), settings), Some(NavMode::Pan));
    assert_eq!(nav_mode(held(NavMode::Zoom), settings), Some(NavMode::Zoom));
    assert_eq!(
        nav_mode(held(NavMode::Rotate), settings),
        Some(NavMode::Rotate)
    );
}

#[test]
fn the_primary_contact_paints_unless_a_held_key_or_the_move_tool_says_otherwise() {
    let settings = Settings::default();
    assert_eq!(nav_mode(press(NavButton::Primary), settings), None);
    let move_tool = NavPress {
        tool_pans: true,
        ..press(NavButton::Primary)
    };
    assert_eq!(nav_mode(move_tool, settings), Some(NavMode::Pan));
}

#[test]
fn pen_buttons_follow_their_bindings() {
    let settings = Settings {
        lower_pen_button: PenButtonAction::Rotate,
        upper_pen_button: PenButtonAction::Zoom,
        ..Settings::default()
    };
    assert_eq!(
        nav_mode(press(NavButton::Middle), settings),
        Some(NavMode::Rotate)
    );
    assert_eq!(
        nav_mode(press(NavButton::Secondary), settings),
        Some(NavMode::Zoom)
    );
    let picks = Settings {
        upper_pen_button: PenButtonAction::PickColour,
        ..settings
    };
    assert_eq!(nav_mode(press(NavButton::Secondary), picks), None);
}

#[test]
fn on_wayland_the_lower_binding_drives_the_secondary_button() {
    let settings = Settings {
        lower_pen_button: PenButtonAction::Rotate,
        upper_pen_button: PenButtonAction::Zoom,
        ..Settings::default()
    };
    let wayland = |button| NavPress {
        pen_buttons: PenButtons::Wayland,
        ..press(button)
    };
    assert_eq!(
        nav_mode(wayland(NavButton::Secondary), settings),
        Some(NavMode::Rotate)
    );
    assert_eq!(
        nav_mode(wayland(NavButton::Middle), settings),
        Some(NavMode::Zoom)
    );
}

#[test]
fn each_backend_numbers_the_barrel_buttons_its_own_way() {
    let settings = Settings {
        lower_pen_button: PenButtonAction::Rotate,
        upper_pen_button: PenButtonAction::Zoom,
        ..Settings::default()
    };
    let lower = Some(PenButtonAction::Rotate);
    let upper = Some(PenButtonAction::Zoom);
    assert_eq!(
        PenButtons::Wayland.action(NavButton::Secondary, settings),
        lower
    );
    assert_eq!(
        PenButtons::Wayland.action(NavButton::Middle, settings),
        upper
    );
    assert_eq!(PenButtons::Other.action(NavButton::Middle, settings), lower);
    assert_eq!(
        PenButtons::Other.action(NavButton::Secondary, settings),
        upper
    );
    for backend in [PenButtons::Wayland, PenButtons::Other] {
        assert_eq!(backend.action(NavButton::Primary, settings), None);
    }
}

#[test]
fn mouse_buttons_ignore_the_pen_bindings() {
    let settings = Settings {
        lower_pen_button: PenButtonAction::Nothing,
        upper_pen_button: PenButtonAction::Rotate,
        ..Settings::default()
    };
    let mouse = |button| NavPress {
        from_pen: false,
        ..press(button)
    };
    assert_eq!(
        nav_mode(mouse(NavButton::Middle), settings),
        Some(NavMode::Pan)
    );
    assert_eq!(nav_mode(mouse(NavButton::Secondary), settings), None);
}

#[test]
fn nothing_navigates_mid_stroke() {
    let stroking = |button| NavPress {
        stroking: true,
        held: Some(NavMode::Pan),
        ..press(button)
    };
    let settings = Settings::default();
    for button in [NavButton::Primary, NavButton::Middle, NavButton::Secondary] {
        assert_eq!(nav_mode(stroking(button), settings), None);
    }
}

#[test]
fn space_pans_ctrl_zooms_and_shift_rotates() {
    let none = gdk::ModifierType::empty();
    let ctrl = gdk::ModifierType::CONTROL_MASK;
    let shift = gdk::ModifierType::SHIFT_MASK;
    assert_eq!(held_mode(false, none), None);
    assert_eq!(held_mode(true, none), Some(NavMode::Pan));
    assert_eq!(held_mode(false, ctrl), Some(NavMode::Zoom));
    assert_eq!(held_mode(false, shift), Some(NavMode::Rotate));
}

#[test]
fn ctrl_and_shift_win_over_space_and_ctrl_wins_over_shift() {
    let ctrl = gdk::ModifierType::CONTROL_MASK;
    let shift = gdk::ModifierType::SHIFT_MASK;
    assert_eq!(held_mode(true, ctrl), Some(NavMode::Zoom));
    assert_eq!(held_mode(true, shift), Some(NavMode::Rotate));
    assert_eq!(held_mode(false, ctrl | shift), Some(NavMode::Zoom));
}

#[test]
fn other_modifiers_hold_nothing() {
    assert_eq!(held_mode(false, gdk::ModifierType::ALT_MASK), None);
}

#[test]
fn colour_picking_follows_the_bindings_for_a_pen_and_the_right_button_for_a_mouse() {
    let settings = Settings {
        lower_pen_button: PenButtonAction::PickColour,
        upper_pen_button: PenButtonAction::Pan,
        ..Settings::default()
    };
    let other = PenButtons::Other;
    assert!(picks_colour(NavButton::Middle, true, other, settings));
    assert!(!picks_colour(NavButton::Secondary, true, other, settings));
    assert!(picks_colour(NavButton::Secondary, false, other, settings));
    assert!(!picks_colour(NavButton::Middle, false, other, settings));
    assert!(!picks_colour(NavButton::Primary, true, other, settings));
    assert!(!picks_colour(NavButton::Primary, false, other, settings));
}

#[test]
fn on_wayland_the_lower_button_picks_through_the_secondary_button() {
    let settings = Settings {
        lower_pen_button: PenButtonAction::PickColour,
        upper_pen_button: PenButtonAction::Pan,
        ..Settings::default()
    };
    let wayland = PenButtons::Wayland;
    assert!(picks_colour(NavButton::Secondary, true, wayland, settings));
    assert!(!picks_colour(NavButton::Middle, true, wayland, settings));
    assert!(picks_colour(NavButton::Secondary, false, wayland, settings));
    assert!(!picks_colour(NavButton::Middle, false, wayland, settings));
}

#[test]
fn gdk_buttons_map_to_nav_buttons() {
    for button in NavButton::ALL {
        assert_eq!(NavButton::from_button(button.button()), Some(button));
    }
    assert_eq!(NavButton::from_button(gdk::BUTTON_MIDDLE + 5), None);
}

#[test]
fn a_pan_drag_steps_by_the_pointer_movement() {
    let mut drag = NavDrag::new(NavMode::Pan, (10.0, 10.0));
    assert_eq!(
        drag.step((5.0, -3.0), (0.0, 0.0)),
        Some(ViewStep::Pan { dx: 5.0, dy: -3.0 })
    );
    assert_eq!(
        drag.step((6.0, -3.0), (0.0, 0.0)),
        Some(ViewStep::Pan { dx: 1.0, dy: 0.0 })
    );
}

#[test]
fn dragging_up_zooms_in_about_the_press_point() {
    let mut drag = NavDrag::new(NavMode::Zoom, (40.0, 30.0));
    let Some(ViewStep::Zoom { anchor, factor }) = drag.step((7.0, -100.0), (0.0, 0.0)) else {
        panic!("a zoom drag zooms");
    };
    assert_eq!(anchor, (40.0, 30.0));
    assert!((factor - (100.0 * ZOOM_PER_DRAG_PX).exp() as f32).abs() < 1e-4);
    let Some(ViewStep::Zoom { factor, .. }) = drag.step((7.0, -50.0), (0.0, 0.0)) else {
        panic!("a zoom drag zooms");
    };
    assert!((factor - (-0.5f64).exp() as f32).abs() < 1e-4);
}

#[test]
fn a_rotate_drag_turns_by_the_angle_swept_around_the_centre() {
    let centre = (100.0, 100.0);
    let mut drag = NavDrag::new(NavMode::Rotate, (200.0, 100.0));
    let Some(ViewStep::Rotate {
        centre: about,
        delta,
    }) = drag.step((-100.0, 100.0), centre)
    else {
        panic!("a rotate drag rotates");
    };
    assert_eq!(about, centre);
    assert!((delta - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
}

#[test]
fn a_rotate_drag_waits_while_the_pointer_is_on_the_centre() {
    let centre = (100.0, 100.0);
    let mut drag = NavDrag::new(NavMode::Rotate, (200.0, 100.0));
    assert_eq!(drag.step((-99.0, 1.0), centre), None);
    let Some(ViewStep::Rotate { delta, .. }) = drag.step((-100.0, 100.0), centre) else {
        panic!("a rotate drag rotates");
    };
    assert!((delta - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
}

#[test]
fn a_rotate_drag_crossing_the_left_takes_the_short_way() {
    let centre = (0.0, 0.0);
    let mut drag = NavDrag::new(NavMode::Rotate, (-100.0, -1.0));
    let Some(ViewStep::Rotate { delta, .. }) = drag.step((0.0, 2.0), centre) else {
        panic!("a rotate drag rotates");
    };
    assert!(delta.abs() < 0.1);
}

#[test]
fn a_rotate_drag_pressed_on_the_centre_rotates_once_it_has_moved_out() {
    let centre = (100.0, 100.0);
    let mut drag = NavDrag::new(NavMode::Rotate, centre);
    assert_eq!(drag.step((100.0, 0.0), centre), None);
    let Some(ViewStep::Rotate { delta, .. }) = drag.step((0.0, 100.0), centre) else {
        panic!("a rotate drag rotates");
    };
    assert!((delta - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
}

#[test]
fn modifiers_follow_ctrl_and_shift_presses_and_releases() {
    let none = gdk::ModifierType::empty();
    let ctrl = gdk::ModifierType::CONTROL_MASK;
    let shift = gdk::ModifierType::SHIFT_MASK;
    assert_eq!(modifiers_after(gdk::Key::Control_L, true, none), ctrl);
    assert_eq!(modifiers_after(gdk::Key::Shift_R, true, ctrl), ctrl | shift);
    assert_eq!(
        modifiers_after(gdk::Key::Control_R, false, ctrl | shift),
        shift
    );
    assert_eq!(modifiers_after(gdk::Key::Shift_L, false, shift), none);
    assert_eq!(modifiers_after(gdk::Key::a, true, ctrl), ctrl);
}

#[test]
fn a_claimed_space_press_keeps_being_swallowed_until_released() {
    assert_eq!(space_press(false, true), SpacePress::Hold);
    assert_eq!(space_press(true, true), SpacePress::Hold);
    assert_eq!(space_press(true, false), SpacePress::Release);
    assert_eq!(space_press(false, false), SpacePress::Pass);
}

#[test]
fn held_keys_show_their_mode_s_cursor_only_when_it_changes() {
    let held = HeldKeys::default();
    let none = gdk::ModifierType::empty();
    assert_eq!(held.set(false, none, "idle"), None);
    assert_eq!(held.set(true, none, "idle"), Some("grab"));
    assert!(held.space_held());
    assert_eq!(held.set(true, none, "idle"), None);
    assert_eq!(
        held.set(true, gdk::ModifierType::CONTROL_MASK, "idle"),
        Some("zoom-in")
    );
    assert_eq!(held.set(false, none, "idle"), Some("idle"));
    assert!(!held.space_held());
}

#[test]
fn held_keys_or_the_contact_s_modifiers_hold_navigation() {
    let held = HeldKeys::default();
    let none = gdk::ModifierType::empty();
    assert!(!held.hold_navigation(none));
    assert!(held.hold_navigation(gdk::ModifierType::SHIFT_MASK));
    held.set(true, none, "idle");
    assert!(held.hold_navigation(none));
}
