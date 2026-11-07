use super::{
    CONTACT_REST_FOLLOW, DabStage, FootprintShape, RAG_STRENGTH_PARAM, Spreading,
    TOOL_MODE_BLENDER, ToolProfile,
};

pub(super) const BLENDER: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_BLENDER,
    spreading: Spreading::AdvectAndDiffuse,
    holder: None,
    footprint: Some(FootprintShape::Bristles),
    rest_follow: CONTACT_REST_FOLLOW,
    dab_stages: &[
        DabStage::Bristles,
        DabStage::Contact,
        DabStage::Exchange,
        DabStage::Apply,
        DabStage::Capillary,
        DabStage::Spread,
    ],
    strength_param: RAG_STRENGTH_PARAM,
};
