use super::*;
use cadmium_engine::palette::{ColourRef, paint_recipe};

fn wheel_key(darken: f32) -> WheelKey {
    WheelKey::of(
        &Palette {
            my_colours: vec![],
            circle: vec![ColourRef::Paint(1)],
        },
        &["titanium_white".to_string(), "ultramarine_blue".to_string()],
        darken,
    )
}

#[test]
fn the_wheel_is_drawn_again_only_when_it_shows_and_what_it_draws_changed() {
    let key = wheel_key(0.25);
    assert!(wheel_is_stale(true, None, &key));
    assert!(!wheel_is_stale(false, None, &key));
    assert!(!wheel_is_stale(true, Some(&key), &wheel_key(0.25)));
    assert!(wheel_is_stale(true, Some(&key), &wheel_key(0.5)));
    assert!(!wheel_is_stale(false, Some(&key), &wheel_key(0.5)));
}

#[test]
fn the_wheel_depends_on_the_circle_the_pinned_colours_the_paints_and_the_darkening() {
    let key = wheel_key(0.25);
    let mut other_circle = key.clone();
    other_circle.palette.circle.push(ColourRef::Paint(0));
    let mut other_pinned = key.clone();
    other_pinned.palette.my_colours.push(paint_recipe(1));
    let mut other_paints = key.clone();
    other_paints.slots.push("ivory_black".to_string());
    for other in [other_circle, other_pinned, other_paints, wheel_key(0.26)] {
        assert_ne!(key, other);
    }
}
