use super::{BLADE_MODES, DabStage, Holder};
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
fn the_knife_is_the_only_tool_with_a_blade() {
    let bladed: Vec<_> = Tool::ALL
        .into_iter()
        .filter(|tool| tool.profile().has_blade_footprint())
        .collect();
    assert_eq!(bladed, [Tool::Knife]);
}

#[test]
fn the_painting_knife_still_carries_its_paint_along_its_edge() {
    let profile = Tool::Knife.profile();
    assert_eq!(profile.holder, Some(Holder::Blade));
    assert!(profile.holds_on_blade());
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

#[test]
fn the_rag_and_the_pencil_never_rest_on_the_surface() {
    for tool in Tool::ALL {
        let rests = !matches!(tool, Tool::Rag | Tool::Pencil | Tool::Eyedropper);
        assert_eq!(tool.profile().rests_on_surface(), rests, "{tool:?}");
    }
}

#[test]
fn only_the_pencil_leaves_the_paint_untouched() {
    for tool in Tool::ALL {
        let profile = tool.profile();
        let untouched = tool == Tool::Pencil || !profile.places_dabs();
        assert_eq!(!profile.touches_paint(), untouched, "{tool:?}");
    }
}

#[test]
fn only_the_brush_the_knife_and_the_blender_change_the_paint_a_tool_holds() {
    let changing: Vec<Tool> = Tool::ALL
        .into_iter()
        .filter(|tool| tool.profile().changes_held_paint())
        .collect();
    assert_eq!(changing, [Tool::Brush, Tool::Knife, Tool::Blender]);
}
