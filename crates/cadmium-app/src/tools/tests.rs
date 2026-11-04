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
