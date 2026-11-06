use cadmium_engine::brush::{BrushSpec, brush_to_json, factory_brushes};

use super::draft::SavePlan;
use super::draft_for;
use crate::state::test_support::painting_state;

#[test]
fn a_worn_factory_brush_is_edited_at_its_suggested_size() {
    let mut state = painting_state();
    state.wear_library_brush(3);
    state.set_brush_size(17.0);
    let draft = draft_for(&state);
    assert_eq!(draft.original(), &factory_brushes()[3]);
    assert_eq!(draft.deletable(), None);
}

#[test]
fn a_worn_user_brush_is_saved_over_itself() {
    let mut state = painting_state();
    let mine = BrushSpec {
        name: "Mine".into(),
        ..factory_brushes()[3].clone()
    };
    let index = state.import_brush(&brush_to_json(&mine)).unwrap().index;
    let draft = draft_for(&state);
    assert_eq!(draft.save_plan(), SavePlan::Replace(index));
    assert_eq!(draft.deletable(), Some(index));
}

#[test]
fn a_brush_in_no_library_is_edited_as_the_current_brush() {
    let mut state = painting_state();
    state.session.brush.head.dome = 0.37;
    let draft = draft_for(&state);
    assert_eq!(draft.original().name, "Current Brush");
    assert_eq!(draft.deletable(), None);
    let plan = draft.save_plan();
    assert!(
        matches!(&plan, SavePlan::AddNew(brush) if brush.name == "Current Brush (copy)"),
        "{plan:?}"
    );
}
