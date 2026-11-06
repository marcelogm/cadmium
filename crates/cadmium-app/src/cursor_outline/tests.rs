use super::*;
use crate::state::test_support::painting_state;

fn outline(semi_axes: [f32; 2]) -> CursorOutline {
    CursorOutline {
        semi_axes,
        rotation_deg: 30.0,
    }
}

#[test]
fn the_radius_is_the_longer_semi_axis() {
    assert_eq!(outline([3.0, 7.0]).radius(), 7.0);
}

#[test]
fn a_tiny_outline_is_hidden() {
    assert!(!outline([1.0, 1.5]).is_visible());
    assert!(outline([1.0, 2.0]).is_visible());
}

#[test]
fn a_dark_ring_hugs_the_light_one_from_outside() {
    let [dark, light] = outline([4.0, 6.0]).rings((10.0, 20.0));
    assert_eq!(dark.semi_axes, [5.0, 7.0]);
    assert_eq!(light.semi_axes, [4.0, 6.0]);
    assert_eq!(dark.colour, OUTLINE_DARK);
    assert_eq!(light.colour, OUTLINE_LIGHT);
    for ring in [&dark, &light] {
        assert_eq!(ring.centre, (10.0, 20.0));
        assert_eq!(ring.rotation_deg, 30.0);
        assert_eq!(ring.width, OUTLINE_WIDTH);
    }
}

#[test]
fn an_upright_brush_outlines_its_diameter() {
    let state = painting_state();
    let to_widget = 2.0;
    let outline = CursorOutline::of_brush(&state, to_widget);
    let diameter = state.session.brush.geometry.max_diameter_px;
    assert_eq!(outline.radius(), diameter / 2.0 * to_widget);
    assert!(outline.is_visible());
}

#[test]
fn a_tilted_pen_turns_the_outline_to_its_azimuth() {
    let mut state = painting_state();
    state.pen.tilt_x = 40.0;
    state.pen.tilt_y = 40.0;
    let outline = CursorOutline::of_brush(&state, 1.0);
    assert!((outline.rotation_deg - 45.0).abs() < 1e-3);
}
