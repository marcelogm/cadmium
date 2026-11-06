use cadmium_engine::brush::{BrushSpec, brush_to_json, factory_brushes};

use std::cell::RefCell;
use std::rc::Rc;

use cadmium_engine::canvas::StateTexture;
use cadmium_engine::stroke::Tool;

use crate::state::test_support::{
    PAINT_ROW_Y, brush_totals, painting_state, present_one_frame, row_samples, stroke,
};
use crate::thumbnails::{ThumbnailCache, ThumbnailKey, next_missing, open_studio, render};

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

#[test]
fn painting_thumbnails_leaves_the_painting_and_the_worn_brush_alone() {
    let mut state = painting_state();
    state.wear_library_brush(3);
    stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    present_one_frame(&mut state);
    let wet_before = state
        .session
        .canvas
        .read_bytes(&state.gpu, StateTexture::WetVol)
        .unwrap();
    let totals_before = brush_totals(&state);
    let geometry_before = state.session.brush.geometry;
    let state = Rc::new(RefCell::new(state));
    let mut studio = open_studio(&state).unwrap();
    let mut cache = ThumbnailCache::<Vec<u8>>::default();
    for _ in 0..2 {
        let brush = next_missing(&state, &cache).unwrap();
        let (key, thumbnail) = render(&state, &mut studio, &brush).unwrap();
        assert_eq!(key, ThumbnailKey::of(&brush));
        cache.insert(key, thumbnail.rgba);
    }
    let state = state.borrow();
    assert_eq!(state.worn_library_brush(), Some(3));
    assert_eq!(state.session.brush.geometry, geometry_before);
    assert_eq!(brush_totals(&state), totals_before);
    let wet_after = state
        .session
        .canvas
        .read_bytes(&state.gpu, StateTexture::WetVol)
        .unwrap();
    assert_eq!(wet_after, wet_before);
}
