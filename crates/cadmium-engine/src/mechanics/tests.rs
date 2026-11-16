use super::*;

const NYLON_MODULUS_PA: f64 = 3.5e9;
const NYLON_RADIUS_M: f64 = 50e-6;
const HAIR_LENGTH_M: f64 = 15e-3;
const TIP_FORCE_N: f64 = 1e-5;
/// Bico et al. 2004: polyester lamellae 100 µm thick in silicone oil.
const BICO_STIFFNESS_N_M: f64 = 5.1e-4;
const BICO_SURFACE_TENSION_N_PER_M: f64 = 20.6e-3;
const BICO_SPACING_M: f64 = 1e-3;
const TOLERANCE: f64 = 1e-3;

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= TOLERANCE * expected.abs()
}

#[test]
fn a_tenth_of_a_millimetre_nylon_hair_has_the_bending_stiffness_of_the_literature() {
    // The literature review derives EI ≈ 1.7e-8 N·m² for d = 0.1 mm, E = 3.5 GPa.
    let stiffness = bending_stiffness(NYLON_MODULUS_PA, NYLON_RADIUS_M);
    assert!(close(stiffness, 1.718e-8), "{stiffness}");
}

#[test]
fn a_light_tip_load_bends_a_nylon_hair_under_a_millimetre() {
    let stiffness = bending_stiffness(NYLON_MODULUS_PA, NYLON_RADIUS_M);
    let deflection = cantilever_tip_deflection(TIP_FORCE_N, HAIR_LENGTH_M, stiffness);
    assert!(close(deflection, 6.548e-4), "{deflection}");
}

#[test]
fn a_nylon_hair_pressed_on_its_tip_buckles_at_a_fifth_of_a_millinewton() {
    // π²EI/(4L²) with EI = 1.718e-8 N·m² and L = 15 mm.
    let stiffness = bending_stiffness(NYLON_MODULUS_PA, NYLON_RADIUS_M);
    let load = euler_buckling_load(stiffness, HAIR_LENGTH_M);
    assert!(close(load, 1.884e-4), "{load}");
}

#[test]
fn the_elastocapillary_length_of_bicos_lamellae_is_fifteen_centimetres() {
    // Bico et al. report L_EC ≈ 150 mm for these lamellae.
    let length = elastocapillary_length(BICO_STIFFNESS_N_M, BICO_SURFACE_TENSION_N_PER_M);
    assert!(close(length, 0.157_34), "{length}");
}

#[test]
fn a_cluster_of_two_closes_at_bicos_pair_length() {
    let l_ec = elastocapillary_length(BICO_STIFFNESS_N_M, BICO_SURFACE_TENSION_N_PER_M);
    let pair = coalescence_length(2, BICO_SPACING_M, l_ec);
    assert!(close(pair, 0.018_27), "{pair}");
}

#[test]
fn cluster_closing_length_grows_with_the_three_quarter_power_of_the_hair_count() {
    let l_ec = elastocapillary_length(BICO_STIFFNESS_N_M, BICO_SURFACE_TENSION_N_PER_M);
    let two = coalescence_length(2, BICO_SPACING_M, l_ec);
    let sixteen = coalescence_length(16, BICO_SPACING_M, l_ec);
    assert!(close(sixteen / two, 8f64.powf(0.75)), "{}", sixteen / two);
}

#[test]
fn a_forty_pixel_ferrule_holds_about_one_hundred_eighty_tenth_millimetre_hairs() {
    let radius_m = metres_from_px(20.0);
    let hairs = real_hair_count(0.7, radius_m, 2.0 * NYLON_RADIUS_M);
    assert!(close(hairs, 179.2), "{hairs}");
}

#[test]
fn a_pixel_is_forty_micrometres() {
    assert!(close(metres_from_px(25.0), 1e-3));
}
