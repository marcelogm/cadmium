use super::{BLADE_MODES, DabStage, Holder, TOOL_MODE_MIXING_KNIFE};
use crate::params::SimParams;
use crate::stroke::Tool;

#[test]
fn only_a_blade_footprint_is_drawn_with_a_blade_shader_mode() {
    for tool in Tool::ALL {
        let profile = tool.profile();
        assert_eq!(
            profile.has_blade_footprint(),
            BLADE_MODES.contains(&profile.shader_mode),
            "{tool:?}"
        );
    }
}

#[test]
fn a_tool_places_dabs_exactly_when_it_has_dab_stages() {
    for tool in Tool::ALL {
        let profile = tool.profile();
        assert_eq!(
            profile.places_dabs(),
            !profile.dab_stages.is_empty(),
            "{tool:?}"
        );
    }
}

#[test]
fn only_the_brush_and_the_blender_hold_bristles() {
    for tool in Tool::ALL {
        let profile = tool.profile();
        let bristled = matches!(tool, Tool::Brush | Tool::Blender);
        assert_eq!(profile.has_bristle_footprint(), bristled, "{tool:?}");
        assert_eq!(profile.moves_bristles(), bristled, "{tool:?}");
        assert_eq!(
            profile.dab_stages.contains(&DabStage::Contact),
            bristled,
            "{tool:?}"
        );
    }
}

#[test]
fn the_mixing_knife_carries_its_paint_on_a_face_of_its_blade() {
    let profile = Tool::MixingKnife.profile();
    assert_eq!(profile.holder, Some(Holder::Face));
    assert!(profile.holds_on_face());
    assert!(!profile.holds_on_blade());
    assert!(profile.has_blade_footprint());
    assert_eq!(profile.shader_mode, TOOL_MODE_MIXING_KNIFE);
}

#[test]
fn the_painting_knife_still_carries_its_paint_along_its_edge() {
    let profile = Tool::Knife.profile();
    assert_eq!(profile.holder, Some(Holder::Blade));
    assert!(profile.holds_on_blade());
    assert!(!profile.holds_on_face());
}

#[test]
fn only_the_face_holder_works_the_face() {
    for tool in Tool::ALL {
        let profile = tool.profile();
        assert_eq!(
            profile.works_the_face(),
            profile.holds_on_face(),
            "{tool:?}"
        );
    }
}

#[test]
fn every_tool_reads_its_strength_from_a_known_param() {
    for tool in Tool::ALL {
        let key = tool.profile().strength_param;
        assert!(
            SimParams::specs().iter().any(|spec| spec.key == key),
            "{tool:?}: {key}"
        );
    }
}

#[test]
fn only_the_pencil_draws_graphite() {
    for tool in Tool::ALL {
        assert_eq!(
            tool.profile().dab_stages.contains(&DabStage::Graphite),
            tool == Tool::Pencil,
            "{tool:?}"
        );
    }
}
