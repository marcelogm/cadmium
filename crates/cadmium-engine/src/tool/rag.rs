use super::{CONTACT_REST_FOLLOW, FootprintShape, Spreading, TOOL_MODE_RAG, ToolProfile};

pub(super) const RAG: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_RAG,
    spreading: Spreading::Advect,
    holder: None,
    footprint: Some(FootprintShape::Ellipse),
    rest_follow: CONTACT_REST_FOLLOW,
    spreads: false,
    refills: false,
    wipes: true,
    blades: false,
};
