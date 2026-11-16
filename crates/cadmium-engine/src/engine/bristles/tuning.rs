use super::material::{HINGES, HairMaterial, hinge_compliances_per_m};
use crate::brush::HairSpec;
use crate::params::SimParams;

pub(super) const BRISTLE_TUNING_BYTES: u64 = std::mem::size_of::<BristleTuning>() as u64;
const UNIFORM_ALIGNMENT_BYTES: u64 = 16;
/// Two vec4 in the shader hold the hinge compliances.
const COMPLIANCE_SLOTS: usize = 8;
const _: () = assert!(BRISTLE_TUNING_BYTES.is_multiple_of(UNIFORM_ALIGNMENT_BYTES));
const _: () = assert!(HINGES <= COMPLIANCE_SLOTS);

/// What the bristle step reads besides the dab: the relief's load bearing and the hairs'
/// bending compliances.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BristleTuning {
    pub(crate) wet_support: f32,
    pub(crate) canvas_tooth_um: f32,
    pub(crate) spare: [f32; 2],
    /// Bending compliance per metre of segment, per hinge, 1/(N·m²).
    pub(crate) hinge_compliance_per_m: [f32; COMPLIANCE_SLOTS],
}

impl BristleTuning {
    pub(crate) fn of(
        params: &SimParams,
        hair: &HairSpec,
        canvas_tooth_um: f32,
        ferrule_radius_px: f32,
        strands: usize,
    ) -> BristleTuning {
        let hinges = hinge_compliances_per_m(&HairMaterial::of(hair), ferrule_radius_px, strands);
        let mut hinge_compliance_per_m = [0.0; COMPLIANCE_SLOTS];
        hinge_compliance_per_m[..HINGES].copy_from_slice(&hinges);
        BristleTuning {
            wet_support: params.get("bristle_wet_support"),
            canvas_tooth_um,
            spare: [0.0; 2],
            hinge_compliance_per_m,
        }
    }
}

#[cfg(test)]
mod tests;
