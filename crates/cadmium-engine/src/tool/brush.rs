use super::{
    CONTACT_REST_FOLLOW, DabStage, FootprintShape, Holder, Spreading, TOOL_MODE_BRUSH, ToolProfile,
};

pub(super) const BRUSH: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_BRUSH,
    spreading: Spreading::Advect,
    holder: Some(Holder::Reservoir),
    footprint: Some(FootprintShape::Ellipse),
    rest_follow: CONTACT_REST_FOLLOW,
    dab_stages: &[
        DabStage::Exchange,
        DabStage::Apply,
        DabStage::Capillary,
        DabStage::Refill,
        DabStage::Spread,
    ],
};
