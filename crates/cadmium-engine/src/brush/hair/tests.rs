use super::HairSpec;

const MATERIALS: [HairSpec; 5] = [
    HairSpec::SYNTHETIC,
    HairSpec::HOG,
    HairSpec::SABLE,
    HairSpec::MONGOOSE,
    HairSpec::SQUIRREL,
];

#[test]
fn the_default_hair_is_todays_tuned_synthetic() {
    assert_eq!(HairSpec::default(), HairSpec::SYNTHETIC);
    assert_eq!(
        HairSpec::SYNTHETIC,
        HairSpec {
            stiffness: 0.5,
            damping: 0.05,
            static_friction: 1.5,
            kinetic_friction: 1.2,
            drag_anisotropy: 0.3,
            bend_memory: 2.0,
            clumping: 0.5,
            wet_point: 0.9,
            point_release: 0.15,
            point_film: 0.25,
            load_limit: 3.0,
            wicking: 0.01,
        }
    );
}

#[test]
fn every_material_is_within_range() {
    for hair in MATERIALS {
        assert_eq!(hair.clamped(), hair, "{hair:?}");
    }
}

#[test]
fn clamping_pulls_values_into_range_and_replaces_nan() {
    let wild = HairSpec {
        stiffness: 3.0,
        point_release: 0.0,
        load_limit: f32::NAN,
        ..HairSpec::HOG
    };
    let tamed = wild.clamped();
    assert_eq!(tamed.stiffness, HairSpec::STIFFNESS_RANGE[1]);
    assert_eq!(tamed.point_release, HairSpec::POINT_RELEASE_RANGE[0]);
    assert_eq!(tamed.load_limit, HairSpec::SYNTHETIC.load_limit);
}
