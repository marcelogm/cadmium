//! The physical hair the quasi-static solve bends, built from today's `HairSpec` until
//! phase 4 of #62 brings `Hair` in physical units. Every number here is an estimate [U]
//! (docs/superpowers/specs/2026-11-16-xpbd-bristles-literature.md).

use super::layout::BRISTLE_VERTICES;
use crate::brush::HairSpec;
use crate::mechanics::{bending_stiffness, metres_from_px, real_hair_count};

/// Hinges per strand: one per edge, the first being the clamp to the ferrule.
pub(crate) const HINGES: usize = BRISTLE_VERTICES - 1;
/// Young's modulus at `HairSpec::stiffness` 0 and 1, so squirrel ≈ 2, sable ≈ 3.4,
/// synthetic ≈ 4 and hog ≈ 6 GPa [U].
const MODULUS_RANGE_PA: [f64; 2] = [1.0e9, 7.0e9];
/// Root diameter of one hair [U].
const HAIR_DIAMETER_M: f64 = 75.0e-6;
/// Tip radius over root radius [U].
const TIP_TAPER: f64 = 0.6;
/// Share of the ferrule's section the hairs fill [U].
const PACKING: f64 = 0.7;
/// The clamp closes the strand, so its hinge bends over half a segment.
const CLAMP_SPAN_SHARE: f64 = 0.5;

/// The physical properties of one hair: modulus, root diameter and tip taper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HairMaterial {
    pub(crate) modulus_pa: f64,
    pub(crate) diameter_m: f64,
    pub(crate) tip_taper: f64,
}

impl HairMaterial {
    /// The material a `HairSpec` stands for.
    pub(crate) fn of(hair: &HairSpec) -> HairMaterial {
        let [soft, stiff] = MODULUS_RANGE_PA;
        HairMaterial {
            modulus_pa: soft + f64::from(hair.stiffness) * (stiff - soft),
            diameter_m: HAIR_DIAMETER_M,
            tip_taper: TIP_TAPER,
        }
    }

    fn radius_at(&self, share: f64) -> f64 {
        0.5 * self.diameter_m * (1.0 - (1.0 - self.tip_taper) * share)
    }
}

/// Each hinge's bending compliance per metre of segment, in 1/(N·m²): the shader
/// multiplies by the strand's segment length to get rad per N·m. A strand stands for a
/// tuft of N_real/N_sim hairs whose EI add, so the head's stiffness does not depend on
/// how many strands are simulated.
pub(crate) fn hinge_compliances_per_m(
    material: &HairMaterial,
    ferrule_radius_px: f32,
    strands: usize,
) -> [f32; HINGES] {
    let real = real_hair_count(
        PACKING,
        metres_from_px(f64::from(ferrule_radius_px)),
        material.diameter_m,
    );
    let hairs = real / strands.max(1) as f64;
    std::array::from_fn(|hinge| {
        let share = hinge as f64 / HINGES as f64;
        let span = if hinge == 0 { CLAMP_SPAN_SHARE } else { 1.0 };
        let stiffness = hairs * bending_stiffness(material.modulus_pa, material.radius_at(share));
        (span / stiffness) as f32
    })
}

#[cfg(test)]
mod tests;
