use super::*;

const AXES: CircleAxes = CircleAxes {
    white: Some(2),
    black: Some(3),
};

fn two_colours() -> Palette {
    Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(0), ColourRef::Paint(1)],
    }
}

fn at(turn: f32, radius: f32, darken: f32) -> Recipe {
    circle_recipe(
        &two_colours(),
        AXES,
        CirclePoint {
            turn,
            radius,
            darken,
        },
    )
    .unwrap()
}

#[test]
fn the_rim_holds_each_colour_pure_and_their_even_mix_halfway_between() {
    assert_eq!(at(0.0, 1.0, 0.0)[..4], [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(at(0.5, 1.0, 0.0)[..4], [0.0, 1.0, 0.0, 0.0]);
    assert_eq!(at(0.25, 1.0, 0.0)[..4], [0.5, 0.5, 0.0, 0.0]);
    assert_eq!(at(0.75, 1.0, 0.0)[..4], [0.5, 0.5, 0.0, 0.0]);
    assert_eq!(at(0.125, 1.0, 0.0)[..4], [0.75, 0.25, 0.0, 0.0]);
}

#[test]
fn towards_the_centre_the_mix_gains_white_and_darkening_adds_black() {
    assert_eq!(at(0.0, 0.5, 0.0)[..4], [0.5, 0.0, 0.5, 0.0]);
    assert_eq!(at(0.0, 0.0, 0.0)[..4], [0.0, 0.0, 1.0, 0.0]);
    assert_eq!(at(0.0, 1.0, 0.5)[..4], [0.5, 0.0, 0.0, 0.5]);
}

#[test]
fn every_point_is_a_whole_recipe() {
    for (turn, radius, darken) in [(0.1, 0.3, 0.2), (0.9, 1.0, 0.8), (0.33, 0.0, 0.0)] {
        assert!((at(turn, radius, darken).iter().sum::<f32>() - 1.0).abs() < 1e-5);
    }
}

#[test]
fn a_painting_without_white_or_black_keeps_the_rim_mix_everywhere() {
    let no_axes = CircleAxes {
        white: None,
        black: None,
    };
    let point = CirclePoint {
        turn: 0.25,
        radius: 0.2,
        darken: 0.7,
    };
    assert_eq!(
        circle_recipe(&two_colours(), no_axes, point).unwrap()[..2],
        [0.5, 0.5]
    );
}

#[test]
fn an_empty_circle_has_no_recipe_and_one_colour_fills_the_rim() {
    let point = CirclePoint {
        turn: 0.6,
        radius: 1.0,
        darken: 0.0,
    };
    assert_eq!(circle_recipe(&Palette::default(), AXES, point), None);
    let one = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(1)],
    };
    assert_eq!(circle_recipe(&one, AXES, point).unwrap()[..2], [0.0, 1.0]);
}

#[test]
fn the_starting_circle_orders_the_coloured_paints_by_hue() {
    let slots: Vec<String> = [
        "ultramarine_blue",
        "cadmium_yellow",
        "titanium_white",
        "cadmium_red",
        "ivory_black",
    ]
    .map(String::from)
    .to_vec();
    assert_eq!(
        default_circle(&PigmentDb::builtin(), &slots),
        [
            ColourRef::Paint(3),
            ColourRef::Paint(1),
            ColourRef::Paint(0)
        ]
    );
    assert_eq!(
        circle_axes(&slots),
        CircleAxes {
            white: Some(2),
            black: Some(4)
        }
    );
}

#[test]
fn a_dropped_colour_lands_between_the_rim_points_nearest_the_drop() {
    assert_eq!(insertion_index(0.0, 4), 0);
    assert_eq!(insertion_index(0.3, 4), 1);
    assert_eq!(insertion_index(0.95, 4), 4);
    assert_eq!(insertion_index(0.5, 0), 0);
    assert_eq!(nearest_position(0.95, 4), Some(0));
    assert_eq!(nearest_position(0.5, 0), None);
}

#[test]
fn the_turn_wraps_around_the_circle_in_both_directions() {
    assert_eq!(at(1.125, 1.0, 0.0), at(0.125, 1.0, 0.0));
    assert_eq!(at(-0.875, 1.0, 0.0), at(0.125, 1.0, 0.0));
    assert_eq!(at(1.0, 1.0, 0.0)[..2], [1.0, 0.0]);
    assert_eq!(at(-1e-9, 1.0, 0.0)[..2], [1.0, 0.0]);
    assert_eq!(insertion_index(-0.75, 4), 1);
    assert_eq!(nearest_position(1.5, 4), Some(2));
}

#[test]
fn a_point_outside_the_disc_or_darker_than_the_limit_is_held_at_the_edge() {
    assert_eq!(at(0.0, 1.5, 0.0), at(0.0, 1.0, 0.0));
    assert_eq!(at(0.0, -0.5, 0.0), at(0.0, 0.0, 0.0));
    assert_eq!(at(0.0, 1.0, 1.0), at(0.0, 1.0, MAX_DARKEN));
    assert_eq!(at(0.0, 1.0, -0.2), at(0.0, 1.0, 0.0));
    assert_eq!(at(0.0, 0.5, 0.5)[..4], [0.25, 0.0, 0.25, 0.5]);
}

#[test]
fn a_pinned_colour_on_the_circle_mixes_by_its_recipe() {
    let mut palette = Palette::default();
    palette.pin(at(0.25, 1.0, 0.0)).unwrap();
    palette.circle = vec![ColourRef::Mine(0), ColourRef::Paint(1)];
    let point = CirclePoint {
        turn: 0.25,
        radius: 1.0,
        darken: 0.0,
    };
    assert_eq!(
        circle_recipe(&palette, AXES, point).unwrap()[..2],
        [0.25, 0.75]
    );
}

#[test]
fn a_circle_naming_a_colour_that_is_gone_has_no_recipe() {
    let palette = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(0), ColourRef::Mine(4)],
    };
    for turn in [0.1, 0.6] {
        let point = CirclePoint {
            turn,
            radius: 1.0,
            darken: 0.0,
        };
        assert_eq!(circle_recipe(&palette, AXES, point), None);
    }
}

#[test]
fn a_painting_with_only_white_and_black_has_an_empty_circle() {
    let slots = ["titanium_white", "zinc_white", "ivory_black"].map(String::from);
    assert_eq!(default_circle(&PigmentDb::builtin(), &slots), []);
    assert_eq!(
        circle_axes(&slots),
        CircleAxes {
            white: Some(0),
            black: Some(2)
        }
    );
    let only_blue = ["ultramarine_blue".to_string()];
    assert_eq!(
        circle_axes(&only_blue),
        CircleAxes {
            white: None,
            black: None
        }
    );
}

#[test]
fn a_green_sits_between_the_yellow_and_the_blue_and_equal_hues_keep_slot_order() {
    let db = PigmentDb::builtin();
    let slots = ["phthalo_green", "ultramarine_blue", "cadmium_yellow"].map(String::from);
    assert_eq!(
        default_circle(&db, &slots),
        [
            ColourRef::Paint(2),
            ColourRef::Paint(0),
            ColourRef::Paint(1)
        ]
    );
    let twins = ["cadmium_red", "cadmium_red"].map(String::from);
    assert_eq!(
        default_circle(&db, &twins),
        [ColourRef::Paint(0), ColourRef::Paint(1)]
    );
}
