use super::{DabStage, FootprintShape, Holder, Spreading, TOOL_MODE_MIXING_KNIFE, ToolProfile};

const MIXING_KNIFE_DRAG: f32 = 0.05;
const MIXING_KNIFE_REST_FOLLOW: f32 = 0.25;

pub(super) const MIXING_KNIFE: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_MIXING_KNIFE,
    spreading: Spreading::Drag(MIXING_KNIFE_DRAG),
    holder: Some(Holder::Face),
    footprint: Some(FootprintShape::Blade),
    rest_follow: MIXING_KNIFE_REST_FOLLOW,
    dab_stages: &[DabStage::Spread, DabStage::Face],
};
