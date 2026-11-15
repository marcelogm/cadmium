use super::*;

#[test]
fn pinning_keeps_a_recipe_normalised_and_refuses_an_empty_one() {
    let mut palette = Palette::default();
    let mut recipe = [0.0; MAX_SLOTS];
    recipe[0] = 3.0;
    recipe[1] = 1.0;
    assert_eq!(palette.pin(recipe), Some(0));
    assert_eq!(palette.my_colours[0][..2], [0.75, 0.25]);
    assert_eq!(palette.pin([0.0; MAX_SLOTS]), None);
}

#[test]
fn unpinning_takes_the_colour_off_the_circle_and_renumbers_the_later_ones() {
    let mut palette = Palette {
        my_colours: vec![paint_recipe(0), paint_recipe(1), paint_recipe(2)],
        circle: vec![
            ColourRef::Mine(2),
            ColourRef::Mine(0),
            ColourRef::Paint(1),
            ColourRef::Mine(1),
        ],
    };
    palette.unpin(1);
    assert_eq!(palette.my_colours.len(), 2);
    assert_eq!(
        palette.circle,
        [ColourRef::Mine(1), ColourRef::Mine(0), ColourRef::Paint(1)]
    );
}

#[test]
fn a_colour_already_on_the_circle_moves_instead_of_appearing_twice() {
    let mut palette = Palette {
        my_colours: vec![],
        circle: vec![
            ColourRef::Paint(0),
            ColourRef::Paint(1),
            ColourRef::Paint(2),
        ],
    };
    palette.insert_in_circle(0, ColourRef::Paint(2));
    assert_eq!(
        palette.circle,
        [
            ColourRef::Paint(2),
            ColourRef::Paint(0),
            ColourRef::Paint(1)
        ]
    );
}

#[test]
fn a_palette_pointing_outside_the_painting_is_invalid() {
    let outside_slot = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(5)],
    };
    let missing_mine = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Mine(0)],
    };
    let twice = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(0), ColourRef::Paint(0)],
    };
    for palette in [outside_slot, missing_mine, twice] {
        assert!(palette.validate(3).is_err());
    }
    assert!(Palette::default().validate(3).is_ok());
}

fn three_paints() -> Palette {
    Palette {
        my_colours: vec![],
        circle: vec![
            ColourRef::Paint(0),
            ColourRef::Paint(1),
            ColourRef::Paint(2),
        ],
    }
}

#[test]
fn a_recipe_with_a_negative_or_unbounded_share_cannot_be_normalised() {
    let mut negative = paint_recipe(0);
    negative[1] = -0.5;
    let mut unbounded = paint_recipe(0);
    unbounded[1] = f32::INFINITY;
    let mut not_a_number = paint_recipe(0);
    not_a_number[1] = f32::NAN;
    for recipe in [negative, unbounded, not_a_number] {
        assert_eq!(normalised(&recipe), None);
    }
}

#[test]
fn a_reference_resolves_to_its_paint_alone_or_to_the_pinned_recipe() {
    let mut palette = Palette::default();
    palette.pin(paint_recipe(3)).unwrap();
    assert_eq!(
        palette.recipe_of(ColourRef::Paint(1)).unwrap()[..3],
        [0.0, 1.0, 0.0]
    );
    assert_eq!(palette.recipe_of(ColourRef::Mine(0)), Some(paint_recipe(3)));
    assert_eq!(palette.recipe_of(ColourRef::Paint(MAX_SLOTS)), None);
    assert_eq!(palette.recipe_of(ColourRef::Mine(1)), None);
}

#[test]
fn unpinning_a_colour_that_is_not_there_changes_nothing() {
    let mut palette = Palette {
        my_colours: vec![paint_recipe(0)],
        circle: vec![ColourRef::Mine(0), ColourRef::Mine(1), ColourRef::Mine(2)],
    };
    let before = palette.clone();
    palette.unpin(1);
    assert_eq!(palette, before);
}

#[test]
fn a_colour_inserted_past_the_end_of_the_circle_lands_last() {
    let mut palette = three_paints();
    palette.insert_in_circle(9, ColourRef::Mine(0));
    assert_eq!(palette.circle[3], ColourRef::Mine(0));
    palette.insert_in_circle(1, ColourRef::Paint(4));
    assert_eq!(palette.circle[1], ColourRef::Paint(4));
    assert_eq!(palette.circle.len(), 5);
}

#[test]
fn a_circle_colour_moves_to_the_position_asked_and_one_outside_stays_put() {
    let mut palette = three_paints();
    palette.move_in_circle(0, 2);
    assert_eq!(
        palette.circle,
        [
            ColourRef::Paint(1),
            ColourRef::Paint(2),
            ColourRef::Paint(0)
        ]
    );
    palette.move_in_circle(1, 9);
    assert_eq!(
        palette.circle,
        [
            ColourRef::Paint(1),
            ColourRef::Paint(0),
            ColourRef::Paint(2)
        ]
    );
    palette.move_in_circle(3, 0);
    assert_eq!(palette.circle.len(), 3);
    assert_eq!(palette.circle[0], ColourRef::Paint(1));
}

#[test]
fn removing_from_the_circle_takes_that_position_and_ignores_one_outside() {
    let mut palette = three_paints();
    palette.remove_from_circle(1);
    assert_eq!(palette.circle, [ColourRef::Paint(0), ColourRef::Paint(2)]);
    palette.remove_from_circle(2);
    assert_eq!(palette.circle.len(), 2);
}

#[test]
fn a_new_palette_has_no_pinned_colour_and_the_default_circle() {
    let slots = ["titanium_white", "cadmium_red"].map(String::from);
    let palette = Palette::starting(&PigmentDb::builtin(), &slots);
    assert!(palette.my_colours.is_empty());
    assert_eq!(palette.circle, [ColourRef::Paint(1)]);
    assert!(palette.validate(slots.len()).is_ok());
}

#[test]
fn a_pinned_recipe_that_is_not_whole_or_uses_a_missing_paint_is_invalid() {
    let with = |recipe: Recipe| Palette {
        my_colours: vec![recipe],
        circle: vec![ColourRef::Mine(0)],
    };
    let mut heavy = paint_recipe(0);
    heavy[1] = 0.01;
    let mut negative = paint_recipe(0);
    negative[0] = 1.5;
    negative[1] = -0.5;
    let mut not_a_number = paint_recipe(0);
    not_a_number[1] = f32::NAN;
    assert!(with(paint_recipe(2)).validate(3).is_ok());
    assert!(with([0.0; MAX_SLOTS]).validate(3).is_err());
    assert!(with(heavy).validate(3).is_err());
    assert!(with(negative).validate(3).is_err());
    assert!(with(not_a_number).validate(3).is_err());
    assert!(with(paint_recipe(3)).validate(3).is_err());
}

#[test]
fn an_invalid_palette_says_what_is_wrong() {
    let outside_slot = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(7)],
    };
    assert_eq!(
        outside_slot.validate(3).unwrap_err(),
        "circle names paint 7, the painting has 3"
    );
    let one_past_the_last = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(3)],
    };
    assert_eq!(
        one_past_the_last.validate(3).unwrap_err(),
        "circle names paint 3, the painting has 3"
    );
}
