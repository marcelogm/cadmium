use super::{
    CONTACT_REST_FOLLOW, RAG_STRENGTH_PARAM, Spreading, TOOL_MODE_EYEDROPPER, ToolProfile,
};

pub(super) const EYEDROPPER: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_EYEDROPPER,
    spreading: Spreading::Advect,
    holder: None,
    footprint: None,
    rest_follow: CONTACT_REST_FOLLOW,
    dab_stages: &[],
    strength_param: RAG_STRENGTH_PARAM,
};
