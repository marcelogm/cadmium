//! Closed-form mechanics of hairs, in SI units: the analytic results the bristle simulation
//! takes its constants from and is tested against. Sources are listed in
//! docs/superpowers/specs/2026-11-16-xpbd-bristles-literature.md.

use crate::engine::RELIEF_UM_PER_PX;

const METRES_PER_MICROMETRE: f64 = 1e-6;
/// Bico et al. 2004, eq. 2: a cluster of N hairs closes when L⁴ = (9/16)·N³·d²·L_EC².
const CLUSTER_CLOSING_FACTOR: f64 = 9.0 / 16.0;

/// Length in metres of `px` canvas pixels, at the scale the canvas relief uses.
pub fn metres_from_px(px: f64) -> f64 {
    px * f64::from(RELIEF_UM_PER_PX) * METRES_PER_MICROMETRE
}

/// Second moment of area of a round hair of `radius_m`: πr⁴/4.
pub fn second_moment_of_area(radius_m: f64) -> f64 {
    std::f64::consts::FRAC_PI_4 * radius_m.powi(4)
}

/// Bending stiffness EI, in N·m², of a round hair of `radius_m` and Young's modulus `modulus_pa`.
pub fn bending_stiffness(modulus_pa: f64, radius_m: f64) -> f64 {
    modulus_pa * second_moment_of_area(radius_m)
}

/// How far, in metres, a clamped hair of `length_m` and stiffness `stiffness_n_m2` bends
/// under `force_n` at its tip (Euler-Bernoulli): FL³/(3EI).
pub fn cantilever_tip_deflection(force_n: f64, length_m: f64, stiffness_n_m2: f64) -> f64 {
    force_n * length_m.powi(3) / (3.0 * stiffness_n_m2)
}

/// The load, in N, at which a hair of `length_m` and stiffness `stiffness_n_m2`, clamped at
/// one end and free at the other, buckles under an axial load on its tip (Euler): π²EI/(4L²).
pub fn euler_buckling_load(stiffness_n_m2: f64, length_m: f64) -> f64 {
    std::f64::consts::PI.powi(2) * stiffness_n_m2 / (4.0 * length_m * length_m)
}

/// Elastocapillary length √(κ/γ), in metres, of lamellae of bending stiffness
/// `stiffness_per_width_n_m` per unit width (κ = Eh³/12, in N·m) in a liquid of surface tension
/// `surface_tension_n_per_m` (Bico et al. 2004). A hair's EI (N·m²) is a different quantity:
/// fibres need their own form, which is not derived here.
pub fn elastocapillary_length(stiffness_per_width_n_m: f64, surface_tension_n_per_m: f64) -> f64 {
    (stiffness_per_width_n_m / surface_tension_n_per_m).sqrt()
}

/// The free length, in metres, at which a cluster of `hairs` hairs set `spacing_m` apart
/// closes: L = ((9/16)·N³·d²·L_EC²)^¼ (Bico et al. 2004, eq. 2; N = 2 is their eq. 1).
pub fn coalescence_length(hairs: u32, spacing_m: f64, elastocapillary_m: f64) -> f64 {
    let n = f64::from(hairs);
    (CLUSTER_CLOSING_FACTOR * n.powi(3) * spacing_m.powi(2) * elastocapillary_m.powi(2)).powf(0.25)
}

/// How many hairs of `hair_diameter_m` fill a ferrule of `ferrule_radius_m` at packing
/// fraction `packing`: φ·πR² / (πd²/4).
pub fn real_hair_count(packing: f64, ferrule_radius_m: f64, hair_diameter_m: f64) -> f64 {
    4.0 * packing * ferrule_radius_m.powi(2) / hair_diameter_m.powi(2)
}

#[cfg(test)]
mod tests;
