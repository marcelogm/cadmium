use super::{
    CONTACT_REST_FOLLOW, DabStage, FootprintShape, PENCIL_STRENGTH_PARAM, Spreading,
    TOOL_MODE_PENCIL, ToolProfile,
};

pub(super) const PENCIL: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_PENCIL,
    spreading: Spreading::Advect,
    holder: None,
    footprint: Some(FootprintShape::Ellipse),
    rest_follow: CONTACT_REST_FOLLOW,
    dab_stages: &[DabStage::Graphite],
    strength_param: PENCIL_STRENGTH_PARAM,
};
