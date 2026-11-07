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
            "pencil",
            "eyedropper"
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
fn the_tube_is_not_a_painting_tool() {
    assert_eq!(UiTool::from_name("tube"), None);
    for tool in UiTool::ALL {
        assert_ne!(tool.profile().on_painting, Press::Squeeze, "{tool:?}");
    }
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
fn no_painting_tool_loads_on_the_painting() {
    for tool in UiTool::ALL {
        assert!(
            !matches!(tool.profile().on_painting, Press::LoadFromPile(_)),
            "{tool:?}"
        );
    }
}

#[test]
fn a_paint_tool_strokes_as_itself() {
    for tool in UiTool::ALL {
        if let Press::Stroke(pressed) = tool.profile().on_painting {
            assert_eq!(UiTool::Paint(pressed), tool);
        }
    }
}

#[test]
fn the_mixing_knife_is_no_header_tool_and_does_nothing_on_the_painting() {
    let mixing_knife = UiTool::Paint(Tool::MixingKnife);
    assert!(!UiTool::ALL.contains(&mixing_knife));
    assert_eq!(UiTool::from_name(mixing_knife.name()), None);
    let profile = mixing_knife.profile();
    assert!(profile.keys.is_empty() && profile.keypad_keys.is_empty());
    assert_eq!(profile.on_painting, Press::Nothing);
}

#[test]
fn the_pencil_strokes_on_the_painting_and_ignores_the_tray() {
    let pencil = UiTool::Paint(Tool::Pencil);
    assert_eq!(pencil.profile().on_painting, Press::Stroke(Tool::Pencil));
    assert_eq!(TrayTool::Load.press(pencil), Press::Nothing);
    assert!(!TrayTool::Load.is_enabled(pencil));
}
