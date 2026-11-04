use super::{CONTACT_REST_FOLLOW, FootprintShape, Spreading, TOOL_MODE_BLENDER, ToolProfile};

pub(super) const BLENDER: ToolProfile = ToolProfile {
    shader_mode: TOOL_MODE_BLENDER,
    spreading: Spreading::AdvectAndDiffuse,
    holder: None,
    footprint: Some(FootprintShape::Ellipse),
    rest_follow: CONTACT_REST_FOLLOW,
    spreads: true,
    refills: false,
    wipes: false,
    blades: false,
};
