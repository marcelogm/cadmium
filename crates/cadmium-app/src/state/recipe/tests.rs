use super::*;
use cadmium_engine::palette::paint_recipe;

#[test]
fn recipe_follows_its_pigments_into_the_new_palette() {
    let old: Vec<String> = ["white", "blue", "red"].map(String::from).to_vec();
    let new: Vec<String> = ["red", "white"].map(String::from).to_vec();
    assert_eq!(
        recipe_for_palette(&[(0, 0.5), (1, 0.2), (2, 0.3)], &old, &new),
        [(1, 0.5), (0, 0.3)]
    );
}

fn one_hot(slot: usize) -> Recipe {
    paint_recipe(slot)
}

#[test]
fn loading_a_colour_replaces_whatever_was_selected() {
    let mut colour = [0.0; MAX_SLOTS];
    colour[1] = 0.75;
    colour[3] = 0.25;
    assert_eq!(
        plan_colour_load(&[(0, 1.0)], &colour, ColourClick::Load),
        (vec![(1, 0.75), (3, 0.25)], ColourDip::CleanAndLoad)
    );
}

#[test]
fn loading_the_colour_already_selected_cleans_and_reloads_it() {
    assert_eq!(
        plan_colour_load(&[(2, 1.0)], &one_hot(2), ColourClick::Load),
        (vec![(2, 1.0)], ColourDip::CleanAndLoad)
    );
}

#[test]
fn mixing_a_colour_in_adds_a_quarter_of_it_on_top() {
    assert_eq!(
        plan_colour_load(&[(0, 1.0)], &one_hot(1), ColourClick::MixIn),
        (vec![(0, 1.0), (1, 0.25)], ColourDip::AddOnTop(one_hot(1)))
    );
    assert_eq!(
        plan_colour_load(&[(0, 2.0), (3, 2.0)], &one_hot(3), ColourClick::MixIn).0,
        [(0, 2.0), (3, 3.0)]
    );
}

#[test]
fn mixing_a_blend_in_adds_each_of_its_paints_in_slot_order() {
    let mut colour = [0.0; MAX_SLOTS];
    colour[1] = 0.5;
    colour[5] = 0.5;
    assert_eq!(
        plan_colour_load(&[(5, 2.0), (3, 2.0)], &colour, ColourClick::MixIn).0,
        [(5, 2.5), (3, 2.0), (1, 0.5)]
    );
}

#[test]
fn mixing_into_a_clean_tool_loads_the_colour() {
    assert_eq!(
        plan_colour_load(&[], &one_hot(2), ColourClick::MixIn),
        (vec![(2, 1.0)], ColourDip::CleanAndLoad)
    );
}

#[test]
fn picked_paint_becomes_a_recipe_of_the_pigments_present() {
    let mut fractions = [0.0; MAX_SLOTS];
    fractions[1] = 0.6;
    fractions[4] = 0.3;
    fractions[5] = 1e-5;
    assert_eq!(recipe_from_fractions(&fractions), [(1, 0.6), (4, 0.3)]);
    assert!(recipe_from_fractions(&[0.0; MAX_SLOTS]).is_empty());
}
