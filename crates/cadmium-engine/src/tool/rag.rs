use super::{CONTACT_REST_FOLLOW, DabStage, FootprintShape, Spreading, TOOL_MODE_RAG, ToolProfile};

pub(super) const RAG: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_RAG,
    spreading: Spreading::Advect,
    holder: None,
    footprint: Some(FootprintShape::Ellipse),
    rest_follow: CONTACT_REST_FOLLOW,
    dab_stages: &[DabStage::Wipe],
};
