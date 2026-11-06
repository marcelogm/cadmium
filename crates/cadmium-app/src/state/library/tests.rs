use cadmium_engine::brush::{BrushSpec, brush_to_json, factory_brushes};

use crate::state::test_support::painting_state;

fn sized(size_px: f32, name: &str) -> String {
    brush_to_json(&BrushSpec {
        name: name.into(),
        size_px,
        ..factory_brushes()[2].clone()
    })
}

#[test]
fn a_re_imported_factory_brush_can_be_picked_and_deleted() {
    let mut state = painting_state();
    let index = state
        .import_brush(&brush_to_json(&factory_brushes()[0]))
        .unwrap()
        .index;
    assert_eq!(state.worn_library_brush(), Some(index));
    assert!(state.brushes.is_user(index));
    state.wear_library_brush(0);
    assert_eq!(state.worn_library_brush(), Some(0));
    state.wear_library_brush(index);
    assert_eq!(state.worn_library_brush(), Some(index));
    state.delete_library_brush(index).unwrap();
    assert_eq!(state.worn_library_brush(), Some(0));
    assert_eq!(state.brushes.all().count(), factory_brushes().len());
}

#[test]
fn deleting_an_earlier_brush_keeps_the_worn_one() {
    let mut state = painting_state();
    let first = state.import_brush(&sized(20.0, "Small")).unwrap().index;
    let second = state.import_brush(&sized(60.0, "Large")).unwrap().index;
    assert_eq!(state.worn_library_brush(), Some(second));
    state.delete_library_brush(first).unwrap();
    let worn = state.worn_library_brush().unwrap();
    assert_eq!(state.brushes.get(worn).unwrap().name, "Large");
    assert_eq!(state.session.brush.geometry.max_diameter_px, 60.0);
}

#[test]
fn wearing_a_brush_from_elsewhere_forgets_the_picked_one() {
    let mut state = painting_state();
    state.import_brush(&sized(20.0, "Small")).unwrap();
    let second = state.import_brush(&sized(60.0, "Large")).unwrap().index;
    let large = state.brushes.get(second).unwrap().clone();
    state.wear_brush(&large);
    assert_eq!(state.worn_library_brush(), Some(2));
}
