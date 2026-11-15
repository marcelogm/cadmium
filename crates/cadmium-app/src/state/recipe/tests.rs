use super::*;

#[test]
fn recipe_follows_its_pigments_into_the_new_palette() {
    let old: Vec<String> = ["white", "blue", "red"].map(String::from).to_vec();
    let new: Vec<String> = ["red", "white"].map(String::from).to_vec();
    assert_eq!(
        recipe_for_palette(&[(0, 0.5), (1, 0.2), (2, 0.3)], &old, &new),
        [(1, 0.5), (0, 0.3)]
    );
}

#[test]
fn clicking_the_only_pigment_again_cleans_and_reloads_it() {
    assert_eq!(
        plan_pigment_dip(&[(2, 1.0)], 2, PigmentClick::Load),
        (vec![(2, 1.0)], PigmentDip::CleanAndLoad)
    );
}

#[test]
fn clicking_another_pigment_leaves_only_that_one() {
    assert_eq!(
        plan_pigment_dip(&[(0, 0.7), (1, 0.3)], 4, PigmentClick::Load),
        (vec![(4, 1.0)], PigmentDip::CleanAndLoad)
    );
}

#[test]
fn clicking_a_pigment_of_the_mix_leaves_only_that_one() {
    assert_eq!(
        plan_pigment_dip(&[(0, 2.0), (3, 1.0)], 3, PigmentClick::Load),
        (vec![(3, 1.0)], PigmentDip::CleanAndLoad)
    );
}

#[test]
fn ctrl_click_adds_a_quarter_of_the_pile_of_that_pigment() {
    assert_eq!(
        plan_pigment_dip(&[(0, 1.0)], 3, PigmentClick::MixIn),
        (vec![(0, 1.0), (3, 0.25)], PigmentDip::AddOnTop(3))
    );
    assert_eq!(
        recipe_after_mixing(&[(0, 2.0), (3, 2.0)], 3),
        [(0, 2.0), (3, 3.0)]
    );
}

#[test]
fn ctrl_click_on_an_empty_palette_starts_a_pile() {
    assert_eq!(recipe_after_mixing(&[], 2), [(2, 1.0)]);
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
