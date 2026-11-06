use cadmium_engine::brush::{HairSpec, HeadSpec, factory_brushes};

use super::{HairPreset, HeadPreset, hair_name, head_name};

#[test]
fn the_head_presets_list_in_the_editors_order() {
    let names = HeadPreset::ALL.map(HeadPreset::name);
    assert_eq!(
        names,
        [
            "Round", "Flat", "Bright", "Filbert", "Fan", "Rigger", "Angular", "Dagger", "Mop"
        ]
    );
}

#[test]
fn the_hair_presets_list_in_the_editors_order() {
    let names = HairPreset::ALL.map(HairPreset::name);
    assert_eq!(
        names,
        [
            "Synthetic",
            "Hog Bristle",
            "Kolinsky Sable",
            "Mongoose",
            "Squirrel"
        ]
    );
}

#[test]
fn each_preset_is_found_again_from_its_values() {
    for preset in HeadPreset::ALL {
        assert_eq!(HeadPreset::of(&preset.spec()), Some(preset));
        assert_eq!(head_name(&preset.spec()), Some(preset.name()));
    }
    for preset in HairPreset::ALL {
        assert_eq!(HairPreset::of(&preset.spec()), Some(preset));
        assert_eq!(hair_name(&preset.spec()), Some(preset.name()));
    }
}

#[test]
fn every_factory_brush_is_made_of_presets() {
    for brush in factory_brushes() {
        assert!(HeadPreset::of(&brush.head).is_some(), "{}", brush.name);
        assert!(HairPreset::of(&brush.hair).is_some(), "{}", brush.name);
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
    assert_eq!(HeadPreset::of(&head), None);
    assert_eq!(head_name(&head), None);
    assert_eq!(hair_name(&hair), None);
}

#[test]
fn each_head_preset_maps_to_its_const() {
    assert_eq!(HeadPreset::Round.spec(), HeadSpec::ROUND);
    assert_eq!(HeadPreset::Flat.spec(), HeadSpec::FLAT);
    assert_eq!(HeadPreset::Bright.spec(), HeadSpec::BRIGHT);
    assert_eq!(HeadPreset::Filbert.spec(), HeadSpec::FILBERT);
    assert_eq!(HeadPreset::Fan.spec(), HeadSpec::FAN);
    assert_eq!(HeadPreset::Rigger.spec(), HeadSpec::RIGGER);
    assert_eq!(HeadPreset::Angular.spec(), HeadSpec::ANGULAR);
    assert_eq!(HeadPreset::Dagger.spec(), HeadSpec::DAGGER);
    assert_eq!(HeadPreset::Mop.spec(), HeadSpec::MOP);
}

#[test]
fn each_hair_preset_maps_to_its_const() {
    assert_eq!(HairPreset::Synthetic.spec(), HairSpec::SYNTHETIC);
    assert_eq!(HairPreset::Hog.spec(), HairSpec::HOG);
    assert_eq!(HairPreset::Sable.spec(), HairSpec::SABLE);
    assert_eq!(HairPreset::Mongoose.spec(), HairSpec::MONGOOSE);
    assert_eq!(HairPreset::Squirrel.spec(), HairSpec::SQUIRREL);
}
