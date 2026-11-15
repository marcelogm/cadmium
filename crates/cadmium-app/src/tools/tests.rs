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
            "pencil",
            "brush",
            "knife",
            "blender",
            "rag",
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
fn every_tool_presses_one_way() {
    let presses: Vec<_> = UiTool::ALL
        .into_iter()
        .map(|tool| tool.profile().press())
        .collect();
    assert_eq!(
        presses,
        [
            Press::Pan,
            Press::Stroke(Tool::Pencil),
            Press::Stroke(Tool::Brush),
            Press::Stroke(Tool::Knife),
            Press::Stroke(Tool::Blender),
            Press::Stroke(Tool::Rag),
            Press::PickColour,
        ]
    );
}

#[test]
fn the_sheet_offers_the_paint_tools_and_no_pencil() {
    let names: Vec<_> = UiTool::SHEET.into_iter().map(UiTool::name).collect();
    assert_eq!(names, ["brush", "knife", "blender", "rag", "eyedropper"]);
}
