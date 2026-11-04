use super::*;

#[test]
fn every_tool_round_trips_through_its_toggle_name() {
    for tool in UiTool::ALL {
        assert_eq!(UiTool::from_name(tool.name()), Some(tool));
    }
}

#[test]
fn toggle_names_match_the_header_tool_group() {
    let names: Vec<_> = UiTool::ALL.iter().map(|tool| tool.name()).collect();
    assert_eq!(
        names,
        [
            "move",
            "brush",
            "knife",
            "blender",
            "rag",
            "eyedropper",
            "tube"
        ]
    );
}

#[test]
fn every_toggle_tooltip_names_the_tool_and_its_first_key() {
    for tool in UiTool::ALL {
        let profile = tool.profile();
        let key = profile.keys[0].name().unwrap().to_uppercase();
        assert!(
            profile
                .tooltip
                .starts_with(&format!("{} ({key}", tool.label())),
            "{tool:?}"
        );
    }
}

#[test]
fn the_tube_is_not_an_engine_tool() {
    assert_eq!(UiTool::from_name(UiTool::Tube.name()), Some(UiTool::Tube));
    assert_eq!(UiTool::Tube.label(), "Tubo");
}

#[test]
fn unknown_toggle_name_is_not_a_tool() {
    assert_eq!(UiTool::from_name("spray"), None);
}

#[test]
fn only_the_knife_explains_how_it_is_held() {
    for tool in UiTool::ALL {
        assert_eq!(
            tool.handling().is_some(),
            tool == UiTool::Paint(Tool::Knife)
        );
    }
}

#[test]
fn app_starts_with_the_brush() {
    assert_eq!(UiTool::default(), UiTool::Paint(Tool::Brush));
}

#[test]
fn a_paint_tool_strokes_and_loads_as_itself() {
    let presses = UiTool::ALL.into_iter().flat_map(|tool| {
        let profile = tool.profile();
        [(tool, profile.on_tray), (tool, profile.on_painting)]
    });
    for (tool, press) in presses {
        if let Press::Stroke(pressed) | Press::LoadFromPile(pressed) = press {
            assert_eq!(UiTool::Paint(pressed), tool);
        }
    }
}

#[test]
fn only_tools_that_hold_paint_load_and_only_from_the_tray() {
    for tool in UiTool::ALL {
        let holds_paint = match tool {
            UiTool::Paint(painted) => painted.profile().holder.is_some(),
            UiTool::Move | UiTool::Tube => false,
        };
        let profile = tool.profile();
        assert_eq!(
            matches!(profile.on_tray, Press::LoadFromPile(_)),
            holds_paint,
            "{tool:?}"
        );
        assert!(
            !matches!(profile.on_painting, Press::LoadFromPile(_)),
            "{tool:?}"
        );
    }
}
