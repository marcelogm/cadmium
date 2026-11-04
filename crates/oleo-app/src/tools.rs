pub use oleo_engine::stroke::Tool;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiTool {
    Move,
    Paint(Tool),
}

impl Default for UiTool {
    fn default() -> Self {
        UiTool::Paint(Tool::Brush)
    }
}

impl UiTool {
    pub const ALL: [UiTool; 6] = [
        UiTool::Move,
        UiTool::Paint(Tool::Brush),
        UiTool::Paint(Tool::Knife),
        UiTool::Paint(Tool::Blender),
        UiTool::Paint(Tool::Rag),
        UiTool::Paint(Tool::Eyedropper),
    ];

    pub fn from_name(name: &str) -> Option<UiTool> {
        UiTool::ALL.into_iter().find(|tool| tool.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            UiTool::Move => "move",
            UiTool::Paint(Tool::Brush) => "brush",
            UiTool::Paint(Tool::Knife) => "knife",
            UiTool::Paint(Tool::Blender) => "blender",
            UiTool::Paint(Tool::Rag) => "rag",
            UiTool::Paint(Tool::Eyedropper) => "eyedropper",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            UiTool::Move => "Mover",
            UiTool::Paint(Tool::Brush) => "Pincel",
            UiTool::Paint(Tool::Knife) => "Espátula",
            UiTool::Paint(Tool::Blender) => "Misturador",
            UiTool::Paint(Tool::Rag) => "Pano",
            UiTool::Paint(Tool::Eyedropper) => "Conta-gotas",
        }
    }
}

#[cfg(test)]
mod tests {
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
            ["move", "brush", "knife", "blender", "rag", "eyedropper"]
        );
    }

    #[test]
    fn unknown_toggle_name_is_not_a_tool() {
        assert_eq!(UiTool::from_name("spray"), None);
    }

    #[test]
    fn app_starts_with_the_brush() {
        assert_eq!(UiTool::default(), UiTool::Paint(Tool::Brush));
    }
}
