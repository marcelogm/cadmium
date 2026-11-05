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
fn round_flat_and_fan_heads_keep_todays_sections_and_point_axes() {
    assert_eq!(HeadSpec::ROUND.section(), Section::Round);
    assert_eq!(HeadSpec::FLAT.section(), Section::Bar);
    assert_eq!(HeadSpec::FAN.section(), Section::Arc);
    assert_eq!(HeadSpec::ROUND.point_axes(), [1.0, 1.0]);
    assert_eq!(HeadSpec::FLAT.point_axes(), [0.0, 1.0]);
    assert_eq!(HeadSpec::FAN.point_axes(), [0.0, 0.0]);
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
    assert_eq!(HeadSpec::ROUND.bulge(), 0.0);
    assert_eq!(HeadSpec::FLAT.bulge(), 0.0);
    assert_eq!(HeadSpec::FAN.bulge(), 0.0);
    assert!(HeadSpec::MOP.bulge() > 0.0);
    let splayed_round = HeadSpec {
        fan_deg: 30.0,
        ..HeadSpec::ROUND
    };
    assert!((splayed_round.bulge() - 0.5).abs() < 1e-6);
}
