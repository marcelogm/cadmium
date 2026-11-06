use cadmium_engine::brush::{HairSpec, HeadSpec};

use super::{HAIRS, HEADS, hair_name, head_name};

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
    for ((hair, _), name) in HAIRS.iter().zip(expected) {
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
