use super::{
    DabStage, FootprintShape, Holder, RAG_STRENGTH_PARAM, Spreading, TOOL_MODE_KNIFE, ToolProfile,
};

const KNIFE_DRAG: f32 = 0.15;
const KNIFE_REST_FOLLOW: f32 = 0.25;

pub(super) const KNIFE: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_KNIFE,
    spreading: Spreading::Drag(KNIFE_DRAG),
    holder: Some(Holder::Blade),
    footprint: Some(FootprintShape::Blade),
    rest_follow: KNIFE_REST_FOLLOW,
    dab_stages: &[DabStage::Spread, DabStage::Blade],
    strength_param: RAG_STRENGTH_PARAM,
};
