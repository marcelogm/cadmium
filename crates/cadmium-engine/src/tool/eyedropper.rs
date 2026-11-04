use super::{CONTACT_REST_FOLLOW, Spreading, TOOL_MODE_EYEDROPPER, ToolProfile};

pub(super) const EYEDROPPER: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_EYEDROPPER,
    spreading: Spreading::Advect,
    rest_follow: CONTACT_REST_FOLLOW,
};
