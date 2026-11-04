use super::{
    CONTACT_REST_FOLLOW, DabStage, FootprintShape, Spreading, TOOL_MODE_BLENDER, ToolProfile,
};

pub(super) const BLENDER: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_BLENDER,
    spreading: Spreading::AdvectAndDiffuse,
    holder: None,
    footprint: Some(FootprintShape::Ellipse),
    rest_follow: CONTACT_REST_FOLLOW,
    dab_stages: &[
        DabStage::Exchange,
        DabStage::Apply,
        DabStage::Capillary,
        DabStage::Spread,
    ],
};
