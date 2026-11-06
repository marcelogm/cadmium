use cadmium_engine::brush::{HairSpec, HeadSpec};

use super::{FACTORY_HAIRS, HEADS, hair_name, hair_tint, head_name};

#[test]
fn every_factory_head_has_its_name() {
    let expected = [
        "Round", "Flat", "Fan", "Bright", "Filbert", "Rigger", "Angular", "Dagger", "Mop",
    ];
    for ((head, _), name) in HEADS.iter().zip(expected) {
        assert_eq!(head_name(head), Some(name));
    }
}

#[test]
fn every_factory_hair_has_its_name() {
    let expected = [
        "Synthetic",
        "Hog Bristle",
        "Kolinsky Sable",
        "Mongoose",
        "Squirrel",
    ];
    for ((hair, ..), name) in FACTORY_HAIRS.iter().zip(expected) {
        assert_eq!(hair_name(hair), Some(name));
    }
}

#[test]
fn modified_specs_have_no_name() {
    let head = HeadSpec {
        thickness: 0.7,
        ..HeadSpec::ROUND
    };
    let hair = HairSpec {
        stiffness: 0.51,
        ..HairSpec::SYNTHETIC
    };
    assert_eq!(head_name(&head), None);
    assert_eq!(hair_name(&hair), None);
}

#[test]
fn factory_hairs_get_their_tint() {
    assert_eq!(hair_tint(&HairSpec::SYNTHETIC), [0.55, 0.38, 0.12]);
    assert_eq!(hair_tint(&HairSpec::HOG), [0.80, 0.72, 0.55]);
    assert_eq!(hair_tint(&HairSpec::SABLE), [0.35, 0.12, 0.05]);
    assert_eq!(hair_tint(&HairSpec::MONGOOSE), [0.30, 0.25, 0.20]);
    assert_eq!(hair_tint(&HairSpec::SQUIRREL), [0.10, 0.07, 0.05]);
}

#[test]
fn custom_hair_takes_the_nearest_stiffness() {
    let stiff = HairSpec {
        stiffness: 0.99,
        ..HairSpec::SYNTHETIC
    };
    assert_eq!(hair_tint(&stiff), hair_tint(&HairSpec::HOG));
}

#[test]
fn a_stiffness_tie_goes_to_the_earlier_factory_hair() {
    let (early, late) = (HairSpec::SABLE, HairSpec::SQUIRREL);
    let middle = (early.stiffness + late.stiffness) / 2.0;
    let hair = HairSpec {
        stiffness: middle,
        ..early
    };
    assert_eq!(
        (early.stiffness - middle).abs(),
        (late.stiffness - middle).abs()
    );
    assert_eq!(hair_tint(&hair), hair_tint(&early));
}
