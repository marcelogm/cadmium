use super::*;
use crate::brush::HairSpec;

const FERRULE_RADIUS_PX: f32 = 20.0;
const STRANDS: usize = 64;

fn compliances(hair: &HairSpec, strands: usize) -> [f32; HINGES] {
    hinge_compliances_per_m(&HairMaterial::of(hair), FERRULE_RADIUS_PX, strands)
}

#[test]
fn a_stiffer_hair_bends_less() {
    let soft = compliances(&HairSpec::SQUIRREL, STRANDS);
    let stiff = compliances(&HairSpec::HOG, STRANDS);
    assert!(
        soft.iter().zip(&stiff).all(|(s, h)| s > h),
        "{soft:?} vs {stiff:?}"
    );
}

#[test]
fn the_modulus_spans_one_to_seven_gigapascals() {
    let at = |stiffness| {
        HairMaterial::of(&HairSpec {
            stiffness,
            ..HairSpec::default()
        })
    };
    assert_eq!(at(0.0).modulus_pa, 1.0e9);
    assert_eq!(at(1.0).modulus_pa, 7.0e9);
}

#[test]
fn the_clamp_hinge_spans_half_a_segment() {
    let material = HairMaterial {
        tip_taper: 1.0,
        ..HairMaterial::of(&HairSpec::default())
    };
    let c = hinge_compliances_per_m(&material, FERRULE_RADIUS_PX, STRANDS);
    assert!((c[1] / c[0] - 2.0).abs() < 1e-5, "{c:?}");
}

#[test]
fn the_tapered_tip_is_softer_by_the_fourth_power_of_its_radius() {
    let material = HairMaterial::of(&HairSpec::default());
    let c = hinge_compliances_per_m(&material, FERRULE_RADIUS_PX, STRANDS);
    let share = |hinge: usize| 1.0 - (1.0 - material.tip_taper) * hinge as f64 / HINGES as f64;
    let want = (share(1) / share(HINGES - 1)).powi(4);
    let got = f64::from(c[HINGES - 1] / c[1]);
    assert!((got / want - 1.0).abs() < 1e-4, "{got} vs {want}");
}

#[test]
fn simulating_twice_the_strands_halves_each_strands_hair_count() {
    let one = compliances(&HairSpec::default(), STRANDS);
    let two = compliances(&HairSpec::default(), 2 * STRANDS);
    assert!(
        one.iter()
            .zip(&two)
            .all(|(a, b)| (b / a - 2.0).abs() < 1e-4)
    );
}
