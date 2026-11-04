use super::TOOL_MODE_KNIFE;
use crate::stroke::Tool;

#[test]
fn only_a_blade_footprint_is_drawn_with_the_knife_shader_mode() {
    for tool in Tool::ALL {
        let profile = tool.profile();
        assert_eq!(
            profile.has_blade_footprint(),
            profile.shader_mode == TOOL_MODE_KNIFE,
            "{tool:?}"
        );
    }
}
