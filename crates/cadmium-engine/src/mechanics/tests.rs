use super::*;

const NYLON_MODULUS_PA: f64 = 3.5e9;
const NYLON_RADIUS_M: f64 = 50e-6;
const HAIR_LENGTH_M: f64 = 15e-3;
const TIP_FORCE_N: f64 = 1e-3;
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
fn a_cantilever_tip_load_and_its_deflection_agree_with_the_beam_stiffness() {
    let stiffness = bending_stiffness(NYLON_MODULUS_PA, NYLON_RADIUS_M);
    let deflection = cantilever_tip_deflection(TIP_FORCE_N, HAIR_LENGTH_M, stiffness);
    let spring_n_per_m = 3.0 * stiffness / HAIR_LENGTH_M.powi(3);
    assert!(
        close(deflection * spring_n_per_m, TIP_FORCE_N),
        "{deflection}"
    );
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
    let pair = (4.5 * BICO_SPACING_M.powi(2) * l_ec.powi(2)).powf(0.25);
    assert!(close(coalescence_length(2, BICO_SPACING_M, l_ec), pair));
}

#[test]
fn larger_clusters_need_longer_hairs_to_close() {
    let l_ec = elastocapillary_length(BICO_STIFFNESS_N_M, BICO_SURFACE_TENSION_N_PER_M);
    let lengths: Vec<f64> = [2, 4, 8, 16]
        .iter()
        .map(|&hairs| coalescence_length(hairs, BICO_SPACING_M, l_ec))
        .collect();
    assert!(
        lengths.windows(2).all(|pair| pair[1] > pair[0]),
        "{lengths:?}"
    );
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
