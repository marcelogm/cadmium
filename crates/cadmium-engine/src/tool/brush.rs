use super::{CONTACT_REST_FOLLOW, Spreading, TOOL_MODE_BRUSH, ToolProfile};

pub(super) const BRUSH: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_BRUSH,
    spreading: Spreading::Advect,
    rest_follow: CONTACT_REST_FOLLOW,
    spreads: true,
    refills: true,
    wipes: false,
    blades: false,
};
