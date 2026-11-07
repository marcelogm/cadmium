use super::{
    CONTACT_REST_FOLLOW, DabStage, FootprintShape, Holder, Spreading, TOOL_MODE_MIXING_KNIFE,
    ToolProfile,
};

/// Lighter than the painting knife's drag: the face, not the push, moves the paint.
const MIXING_KNIFE_DRAG: f32 = 0.05;

/// A tray knife with a broad blade whose face carries a 2D layer of paint.
pub(super) const MIXING_KNIFE: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_MIXING_KNIFE,
    spreading: Spreading::Drag(MIXING_KNIFE_DRAG),
    holder: Some(Holder::Face),
    footprint: Some(FootprintShape::Blade),
    rest_follow: CONTACT_REST_FOLLOW,
    dab_stages: &[DabStage::Spread, DabStage::Face],
};
