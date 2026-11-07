use super::*;
use crate::tools::panel::PanelTool;

#[test]
fn names_round_trip_and_match_the_tray_header() {
    let names: Vec<_> = TrayTool::ALL.iter().map(|tool| tool.name()).collect();
    assert_eq!(
        names,
        [
            "load",
            "tray-mixing-knife",
            "tray-blender",
            "tray-rag",
            "tray-tube"
        ]
    );
    for tool in TrayTool::ALL {
        assert_eq!(TrayTool::from_name(tool.name()), Some(tool));
    }
    assert_eq!(TrayTool::from_name("brush"), None);
}

#[test]
fn the_tray_starts_on_load() {
    assert_eq!(TrayTool::default(), TrayTool::Load);
}

#[test]
fn load_loads_the_painting_tool_that_holds_paint() {
    assert_eq!(
        TrayTool::Load.press(UiTool::Paint(Tool::Brush)),
        Press::LoadFromPile(Tool::Brush)
    );
    assert_eq!(
        TrayTool::Load.press(UiTool::Paint(Tool::Knife)),
        Press::LoadFromPile(Tool::Knife)
    );
    for tool in [
        UiTool::Move,
        UiTool::Paint(Tool::Blender),
        UiTool::Paint(Tool::Rag),
        UiTool::Paint(Tool::Eyedropper),
    ] {
        assert_eq!(TrayTool::Load.press(tool), Press::Nothing, "{tool:?}");
        assert!(!TrayTool::Load.is_enabled(tool), "{tool:?}");
    }
}

#[test]
fn the_other_tray_tools_ignore_the_painting_tool() {
    for painting in UiTool::ALL {
        assert_eq!(
            TrayTool::Blender.press(painting),
            Press::Stroke(Tool::Blender)
        );
        assert_eq!(TrayTool::Rag.press(painting), Press::Stroke(Tool::Rag));
        assert_eq!(TrayTool::Tube.press(painting), Press::Squeeze);
        assert_eq!(
            TrayTool::MixingKnife.press(painting),
            Press::Stroke(Tool::MixingKnife)
        );
        for tool in [
            TrayTool::MixingKnife,
            TrayTool::Blender,
            TrayTool::Rag,
            TrayTool::Tube,
        ] {
            assert!(tool.is_enabled(painting));
        }
    }
}

#[test]
fn load_is_named_after_what_it_loads() {
    assert_eq!(
        TrayTool::Load.label_for(UiTool::Paint(Tool::Brush)),
        "Load Brush"
    );
    assert_eq!(
        TrayTool::Load.label_for(UiTool::Paint(Tool::Knife)),
        "Load Knife"
    );
    assert_eq!(TrayTool::Load.label_for(UiTool::Move), "Load");
    assert_eq!(TrayTool::Rag.label_for(UiTool::Paint(Tool::Brush)), "Rag");
}

#[test]
fn load_wears_the_icon_of_what_it_loads() {
    assert_eq!(
        TrayTool::Load.icon_for(UiTool::Paint(Tool::Knife)),
        "cadmium-knife-symbolic"
    );
    assert_eq!(
        TrayTool::Load.icon_for(UiTool::Paint(Tool::Brush)),
        "cadmium-brush-symbolic"
    );
    assert_eq!(
        TrayTool::Load.icon_for(UiTool::Move),
        "cadmium-brush-symbolic"
    );
}

#[test]
fn tray_keys_are_shifted_digits_and_tooltips_name_them() {
    let keys: Vec<_> = TrayTool::ALL.iter().map(|tool| tool.key()).collect();
    assert_eq!(
        keys,
        [
            gdk::Key::_1,
            gdk::Key::_2,
            gdk::Key::_4,
            gdk::Key::_5,
            gdk::Key::_6
        ]
    );
    for tool in TrayTool::ALL {
        let digit = tool.key().name().unwrap();
        assert!(
            tool.profile().tooltip.contains(&format!("(Shift+{digit})")),
            "{tool:?}"
        );
    }
}

#[test]
fn every_tray_tool_shows_in_the_panel() {
    let brush = UiTool::Paint(Tool::Brush);
    assert_eq!(
        TrayTool::Load.panel_tool(brush, Tool::Brush),
        PanelTool::Paint(Tool::Brush)
    );
    assert_eq!(
        TrayTool::Load.panel_tool(UiTool::Move, Tool::Knife),
        PanelTool::Paint(Tool::Knife)
    );
    assert_eq!(
        TrayTool::Blender.panel_tool(brush, Tool::Brush),
        PanelTool::Paint(Tool::Blender)
    );
    assert_eq!(
        TrayTool::Rag.panel_tool(brush, Tool::Brush),
        PanelTool::Paint(Tool::Rag)
    );
    assert_eq!(
        TrayTool::Tube.panel_tool(brush, Tool::Brush),
        PanelTool::Tube
    );
    assert_eq!(
        TrayTool::MixingKnife.panel_tool(brush, Tool::Brush),
        PanelTool::Paint(Tool::MixingKnife)
    );
}

#[test]
fn the_tray_tube_keeps_the_tube_wording() {
    assert_eq!(PanelTool::Tube.label(), "Tube");
    assert_eq!(PanelTool::Tube.wording().size_title, "Nozzle");
    assert!(PanelTool::Tube.controls().pigments);
}

#[test]
fn a_disabled_load_says_what_to_pick_to_use_it() {
    let brush = UiTool::Paint(Tool::Brush);
    assert_eq!(TrayTool::Load.tooltip_for(brush), LOAD.tooltip);
    let disabled = TrayTool::Load.tooltip_for(UiTool::Paint(Tool::Blender));
    assert_ne!(disabled, LOAD.tooltip);
    assert!(disabled.contains("Shift+1"), "{disabled}");
    assert!(disabled.contains("brush or the knife"), "{disabled}");
    for tool in [TrayTool::Blender, TrayTool::Rag, TrayTool::Tube] {
        assert_eq!(tool.tooltip_for(UiTool::Move), tool.profile().tooltip);
    }
}

#[test]
fn the_mixing_knife_is_the_tray_header_s_second_tool() {
    let labels: Vec<_> = TrayTool::ALL.iter().map(|tool| tool.label()).collect();
    assert_eq!(labels, ["Load", "Mixing Knife", "Blender", "Rag", "Tube"]);
    let tool = TrayTool::MixingKnife;
    assert_eq!(tool.name(), "tray-mixing-knife");
    assert_eq!(tool.key(), gdk::Key::_2);
    assert!(tool.profile().tooltip.contains("(Shift+2)"));
    assert_eq!(
        tool.icon_for(UiTool::Paint(Tool::Brush)),
        "cadmium-mixing-knife-symbolic"
    );
    assert_eq!(tool.label_for(UiTool::Move), "Mixing Knife");
}
