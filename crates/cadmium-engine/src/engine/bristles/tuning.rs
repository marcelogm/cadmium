use crate::brush::{HairSpec, HeadSpec};
use crate::params::SimParams;

pub(super) const BRISTLE_TUNING_BYTES: u64 = std::mem::size_of::<BristleTuning>() as u64;
const UNIFORM_ALIGNMENT_BYTES: u64 = 16;
const _: () = assert!(BRISTLE_TUNING_BYTES.is_multiple_of(UNIFORM_ALIGNMENT_BYTES));

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BristleTuning {
    pub(crate) static_friction: f32,
    pub(crate) kinetic_friction: f32,
    pub(crate) drag_anisotropy: f32,
    pub(crate) plasticity: f32,
    pub(crate) clumping: f32,
    pub(crate) pointing: f32,
    pub(crate) point_release: f32,
    pub(crate) point_axes: [f32; 2],
    pub(crate) point_film: f32,
    pub(crate) wet_support: f32,
    pub(crate) canvas_tooth_um: f32,
}

impl BristleTuning {
    pub(crate) fn of(
        params: &SimParams,
        head: &HeadSpec,
        hair: &HairSpec,
        canvas_tooth_um: f32,
    ) -> BristleTuning {
        BristleTuning {
            static_friction: hair.static_friction,
            kinetic_friction: hair.kinetic_friction.min(hair.static_friction),
            drag_anisotropy: hair.drag_anisotropy,
            plasticity: hair.bend_memory,
            clumping: hair.clumping,
            pointing: hair.wet_point,
            point_release: hair.point_release,
            point_axes: head.point_axes(),
            point_film: hair.point_film,
            wet_support: params.get("bristle_wet_support"),
            canvas_tooth_um,
        }
    }
}

#[cfg(test)]
mod tests;
