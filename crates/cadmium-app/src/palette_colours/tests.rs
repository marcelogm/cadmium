use super::*;

#[test]
fn the_selected_colour_highlights_the_paint_or_pinned_colour_it_equals() {
    let mut mix = [0.0; MAX_SLOTS];
    mix[0] = 0.5;
    mix[1] = 0.5;
    let palette = Palette {
        my_colours: vec![mix],
        circle: vec![],
    };
    assert_eq!(
        highlighted_colour(&palette, Some(&paint_recipe(1))),
        Some(ColourRef::Paint(1))
    );
    assert_eq!(
        highlighted_colour(&palette, Some(&mix)),
        Some(ColourRef::Mine(0))
    );
    let mut other = mix;
    other[0] = 0.4;
    other[1] = 0.6;
    assert_eq!(highlighted_colour(&palette, Some(&other)), None);
    assert_eq!(highlighted_colour(&palette, None), None);
}

#[test]
fn a_pinned_colour_that_is_one_paint_highlights_the_paint() {
    let palette = Palette {
        my_colours: vec![paint_recipe(2)],
        circle: vec![],
    };
    assert_eq!(
        highlighted_colour(&palette, Some(&paint_recipe(2))),
        Some(ColourRef::Paint(2))
    );
}

#[test]
fn a_colour_within_a_thousandth_of_a_pinned_one_per_paint_is_that_colour() {
    let mut mix = [0.0; MAX_SLOTS];
    mix[0] = 0.5;
    mix[1] = 0.5;
    let palette = Palette {
        my_colours: vec![paint_recipe(3), mix],
        circle: vec![],
    };
    let mut near = mix;
    near[0] = 0.5005;
    near[1] = 0.4995;
    assert_eq!(
        highlighted_colour(&palette, Some(&near)),
        Some(ColourRef::Mine(1))
    );
    let mut far = mix;
    far[0] = 0.502;
    far[1] = 0.498;
    assert_eq!(highlighted_colour(&palette, Some(&far)), None);
    let mut last_differs = mix;
    last_differs[0] = 0.4;
    last_differs[MAX_SLOTS - 1] = 0.1;
    assert_eq!(highlighted_colour(&palette, Some(&last_differs)), None);
}

#[test]
fn ctrl_mixes_a_colour_in_and_a_plain_click_loads_it() {
    assert_eq!(
        colour_click(gdk::ModifierType::CONTROL_MASK),
        ColourClick::MixIn
    );
    assert_eq!(
        colour_click(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK),
        ColourClick::MixIn
    );
    assert_eq!(
        colour_click(gdk::ModifierType::SHIFT_MASK),
        ColourClick::Load
    );
    assert_eq!(colour_click(gdk::ModifierType::empty()), ColourClick::Load);
}
