use super::{FirstRest, Placement};
use crate::stroke::Tool;

const RESTING_TOOLS: [Tool; 3] = [Tool::Brush, Tool::Knife, Tool::Blender];

fn placed(tools: &[Tool]) -> Vec<Placement> {
    tools
        .iter()
        .map(|tool| {
            let profile = tool.profile();
            let mut record: super::DabRecord = bytemuck::Zeroable::zeroed();
            record.rest_follow = profile.rest_follow;
            Placement {
                record,
                canvas_rect: [0.0; 4],
                canvas_groups: [1; 2],
                tip_groups: [1; 2],
                spread_groups: [1; 2],
                profile,
            }
        })
        .collect()
}

fn first_rests(first_rest: &mut FirstRest, tools: &[Tool]) -> Vec<u32> {
    let mut placements = placed(tools);
    first_rest.take(&mut placements);
    placements
        .iter()
        .map(|placement| placement.record.first_rest)
        .collect()
}

#[test]
fn a_new_engine_marks_the_first_resting_dab_it_places() {
    for tool in RESTING_TOOLS {
        assert_eq!(first_rests(&mut FirstRest::new(), &[tool; 3]), [1, 0, 0]);
    }
}

#[test]
fn only_the_first_resting_dab_of_each_stroke_is_its_first_rest() {
    for tool in RESTING_TOOLS {
        let mut first_rest = FirstRest::new();
        first_rest.begin_stroke();
        assert_eq!(
            first_rests(&mut first_rest, &[tool; 3]),
            [1, 0, 0],
            "{tool:?}"
        );
        assert_eq!(first_rests(&mut first_rest, &[tool; 2]), [0, 0], "{tool:?}");
        first_rest.begin_stroke();
        assert_eq!(first_rests(&mut first_rest, &[tool; 2]), [1, 0], "{tool:?}");
    }
}

#[test]
fn the_first_rest_keeps_the_tool_s_own_follow() {
    for tool in RESTING_TOOLS {
        let mut placements = placed(&[tool]);
        FirstRest::new().take(&mut placements);
        assert_eq!(placements[0].record.rest_follow, tool.profile().rest_follow);
    }
}

#[test]
fn wiping_dabs_leave_the_first_rest_to_the_next_resting_dab() {
    let mut first_rest = FirstRest::new();
    assert_eq!(first_rests(&mut first_rest, &[Tool::Rag; 2]), [0, 0]);
    assert_eq!(
        first_rests(&mut first_rest, &[Tool::Rag, Tool::Brush, Tool::Brush]),
        [0, 1, 0]
    );
    assert_eq!(first_rests(&mut first_rest, &[Tool::Brush]), [0]);
}
