use super::{CONTACT_REST_FOLLOW, Spreading, TOOL_MODE_RAG, ToolProfile};

pub(super) const RAG: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_RAG,
    spreading: Spreading::Advect,
    rest_follow: CONTACT_REST_FOLLOW,
    spreads: false,
    refills: false,
    wipes: true,
    blades: false,
};
