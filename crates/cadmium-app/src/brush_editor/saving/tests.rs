use cadmium_engine::brush::{BrushSpec, brush_to_json, factory_brushes};

use super::{saved_toast, store, store_copy};
use crate::brush_editor::draft_for;
use crate::brush_library::{LibraryError, Saved};
use crate::state::test_support::painting_state;

fn saved(name: &str, renamed: bool, save_error: Option<std::io::Error>) -> Saved {
    Saved {
        index: 0,
        name: name.into(),
        renamed,
        save_error,
    }
}

#[test]
fn the_toast_names_the_saved_brush() {
    assert_eq!(saved_toast(&saved("Mine", false, None)), "Saved Mine");
}

#[test]
fn a_renamed_brush_is_reported_under_its_new_name() {
    assert_eq!(
        saved_toast(&saved("Mine (2)", true, None)),
        "Saved as Mine (2)"
    );
}

#[test]
fn an_unwritten_brush_says_it_lasts_until_quitting() {
    let toast = saved_toast(&saved("Mine", false, Some(std::io::Error::other("full"))));
    assert!(toast.starts_with("Saved Mine; "), "{toast}");
    assert!(toast.contains("until you quit"), "{toast}");
}

#[test]
fn saving_a_factory_draft_adds_a_copy_and_wears_it() {
    let mut state = painting_state();
    let mut draft = draft_for(&state);
    draft.set_size(50.0);
    let saved = store(&mut state, &draft).unwrap();
    assert_eq!(saved.name, "Round · Synthetic (copy)");
    assert_eq!(state.worn_library_brush(), Some(saved.index));
    assert_eq!(state.session.brush.geometry.max_diameter_px, 50.0);
    assert_eq!(state.brushes.get(0), Some(&factory_brushes()[0]));
}

#[test]
fn saving_a_user_draft_replaces_it_in_place() {
    let mut state = painting_state();
    let mine = BrushSpec {
        name: "Mine".into(),
        ..factory_brushes()[3].clone()
    };
    let index = state.import_brush(&brush_to_json(&mine)).unwrap().index;
    let count = state.brushes.all().count();
    let mut draft = draft_for(&state);
    draft.set_size(91.0);
    let saved = store(&mut state, &draft).unwrap();
    assert_eq!(saved.index, index);
    assert_eq!(state.brushes.all().count(), count);
    assert_eq!(state.brushes.get(index).unwrap().size_px, 91.0);
}

#[test]
fn a_draft_whose_brush_was_deleted_meanwhile_is_not_saved() {
    let mut state = painting_state();
    let index = state
        .import_brush(&brush_to_json(&BrushSpec {
            name: "Mine".into(),
            ..factory_brushes()[3].clone()
        }))
        .unwrap()
        .index;
    let draft = draft_for(&state);
    state.delete_library_brush(index).unwrap();
    assert!(matches!(
        store(&mut state, &draft),
        Err(LibraryError::Changed)
    ));
}

#[test]
fn a_copy_of_a_copy_is_numbered_not_copied_twice() {
    let mut state = painting_state();
    let copy = BrushSpec {
        name: "Mine (copy)".into(),
        ..factory_brushes()[3].clone()
    };
    state.import_brush(&brush_to_json(&copy)).unwrap();
    let draft = draft_for(&state);
    let saved = store_copy(&mut state, &draft).unwrap();
    assert_eq!(saved.name, "Mine (copy) (2)");
    assert!(saved.renamed);
}
