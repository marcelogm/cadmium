use crate::brush::BrushShape;
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
    pub(crate) fn of(params: &SimParams, shape: BrushShape, canvas_tooth_um: f32) -> BristleTuning {
        let static_friction = params.get("bristle_static_friction");
        BristleTuning {
            static_friction,
            kinetic_friction: params.get("bristle_kinetic_friction").min(static_friction),
            drag_anisotropy: params.get("bristle_drag_anisotropy"),
            plasticity: params.get("bristle_plasticity"),
            clumping: params.get("bristle_clumping"),
            pointing: params.get("bristle_pointing"),
            point_release: params.get("bristle_point_release"),
            point_axes: point_axes(shape),
            point_film: params.get("bristle_point_film"),
            wet_support: params.get("bristle_wet_support"),
            canvas_tooth_um,
        }
    }
}

fn point_axes(shape: BrushShape) -> [f32; 2] {
    match shape {
        BrushShape::Round => [1.0, 1.0],
        BrushShape::Flat => [0.0, 1.0],
        BrushShape::Fan => [0.0, 0.0],
    }
}

#[cfg(test)]
mod tests;
