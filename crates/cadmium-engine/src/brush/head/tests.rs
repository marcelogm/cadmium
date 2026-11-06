use super::{HeadSpec, Section};

const FACTORY_HEADS: [HeadSpec; 9] = [
    HeadSpec::ROUND,
    HeadSpec::FLAT,
    HeadSpec::BRIGHT,
    HeadSpec::FILBERT,
    HeadSpec::FAN,
    HeadSpec::RIGGER,
    HeadSpec::ANGULAR,
    HeadSpec::DAGGER,
    HeadSpec::MOP,
];

#[test]
fn every_factory_head_is_within_range() {
    for head in FACTORY_HEADS {
        assert_eq!(head.clamped(), head, "{head:?}");
    }
}

#[test]
fn clamping_pulls_values_into_range_and_replaces_nan() {
    let wild = HeadSpec {
        thickness: 0.0,
        length: 99.0,
        dome: -1.0,
        slant: f32::NAN,
        ..HeadSpec::FLAT
    };
    let tamed = wild.clamped();
    assert_eq!(tamed.thickness, HeadSpec::THICKNESS_RANGE[0]);
    assert_eq!(tamed.length, HeadSpec::LENGTH_RANGE[1]);
    assert_eq!(tamed.dome, 0.0);
    assert_eq!(tamed.slant, HeadSpec::ROUND.slant);
}

#[test]
fn every_factory_head_lays_its_roots_in_one_section() {
    use Section::{Arc, Bar, Round};
    let expected = [Round, Bar, Bar, Bar, Arc, Round, Bar, Bar, Round];
    for (head, section) in FACTORY_HEADS.iter().zip(expected) {
        let parts: Vec<_> = head.section_mix().parts().collect();
        assert_eq!(parts, [(section, 1.0)], "{head:?}");
    }
}

#[test]
fn round_flat_and_fan_heads_keep_todays_point_axes() {
    assert_eq!(HeadSpec::ROUND.point_axes(), [1.0, 1.0]);
    assert_eq!(HeadSpec::FLAT.point_axes(), [0.0, 1.0]);
    assert_eq!(HeadSpec::FAN.point_axes(), [0.0, 0.0]);
}

#[test]
fn a_nearly_round_head_is_half_bar_half_round() {
    let mix = HeadSpec {
        thickness: 0.9,
        ..HeadSpec::FLAT
    }
    .section_mix();
    assert!((mix.round - 0.5).abs() < 1e-5, "{mix:?}");
    assert!((mix.bar - 0.5).abs() < 1e-5, "{mix:?}");
    assert_eq!(mix.arc, 0.0);
}

#[test]
fn a_slight_fan_is_half_bar_half_arc() {
    let mix = HeadSpec {
        fan_deg: 5.0,
        ..HeadSpec::FLAT
    }
    .section_mix();
    assert_eq!(mix.round, 0.0);
    assert!((mix.arc - 0.5).abs() < 1e-5, "{mix:?}");
    assert!((mix.bar - 0.5).abs() < 1e-5, "{mix:?}");
}

#[test]
fn the_section_shares_always_sum_to_one() {
    for thickness in [0.08, 0.5, 0.8, 0.85, 0.95, 0.999, 1.0] {
        for fan_deg in [0.0, 0.01, 3.0, 9.99, 10.0, 50.0, 90.0] {
            let mix = HeadSpec {
                thickness,
                fan_deg,
                ..HeadSpec::FLAT
            }
            .section_mix();
            let total = mix.round + mix.bar + mix.arc;
            assert!((total - 1.0).abs() < 1e-6, "{thickness} {fan_deg} {mix:?}");
        }
    }
}

#[test]
fn a_nan_control_counts_as_its_low_end() {
    let mix = HeadSpec {
        thickness: f32::NAN,
        fan_deg: f32::NAN,
        ..HeadSpec::FLAT
    }
    .section_mix();
    assert_eq!(mix.parts().collect::<Vec<_>>(), [(Section::Bar, 1.0)]);
}

#[test]
fn point_scales_the_point_axes() {
    let half = HeadSpec {
        point: 0.5,
        ..HeadSpec::ROUND
    };
    assert_eq!(half.point_axes(), [0.5, 0.5]);
}

#[test]
fn only_a_full_thickness_head_is_round() {
    assert!(HeadSpec::ROUND.is_round() && HeadSpec::MOP.is_round());
    assert!(!HeadSpec::FILBERT.is_round() && !HeadSpec::FAN.is_round());
}

#[test]
fn a_square_head_keeps_full_length_everywhere() {
    for root in [[0.0, 0.0], [1.0, 0.0], [-1.0, 0.2], [0.7, -0.7]] {
        assert_eq!(HeadSpec::FLAT.length_share(root), 1.0);
    }
}

#[test]
fn a_dome_shortens_the_edge_against_the_centre() {
    let domed = HeadSpec {
        dome: 1.0,
        ..HeadSpec::FLAT
    };
    assert_eq!(domed.length_share([0.0, 0.0]), 1.0);
    assert!(domed.length_share([0.9, 0.0]) < 0.5);
}

#[test]
fn a_slant_grows_the_hairs_from_one_side_to_the_other() {
    let slanted = HeadSpec {
        slant: 0.8,
        ..HeadSpec::FLAT
    };
    let shares: Vec<f32> = [-1.0, -0.5, 0.0, 0.5, 1.0]
        .iter()
        .map(|&x| slanted.length_share([x, 0.0]))
        .collect();
    assert!(
        shares.windows(2).all(|pair| pair[1] > pair[0]),
        "{shares:?}"
    );
    assert_eq!(shares[4], 1.0);
    let mirrored = HeadSpec {
        slant: -0.8,
        ..HeadSpec::FLAT
    };
    assert_eq!(mirrored.length_share([-1.0, 0.0]), 1.0);
}

#[test]
fn no_hair_gets_shorter_than_the_minimum_share() {
    let extreme = HeadSpec {
        dome: 1.0,
        slant: 1.0,
        ..HeadSpec::FLAT
    };
    assert!(extreme.length_share([-1.0, 0.0]) >= super::MIN_LENGTH_SHARE);
}

#[test]
fn only_belly_or_a_round_fan_bulges() {
    assert_eq!(HeadSpec::ROUND.bulge(Section::Round), 0.0);
    assert_eq!(HeadSpec::FLAT.bulge(Section::Bar), 0.0);
    assert_eq!(HeadSpec::FAN.bulge(Section::Arc), 0.0);
    assert!(HeadSpec::MOP.bulge(Section::Round) > 0.0);
    let splayed_round = HeadSpec {
        fan_deg: 30.0,
        ..HeadSpec::ROUND
    };
    assert!((splayed_round.bulge(Section::Round) - 0.5).abs() < 1e-6);
    assert_eq!(splayed_round.bulge(Section::Bar), 0.0);
}
