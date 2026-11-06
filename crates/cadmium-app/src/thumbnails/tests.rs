use std::cell::RefCell;
use std::rc::Rc;

use cadmium_engine::brush::factory_brushes;
use cadmium_engine::canvas::StateTexture;
use cadmium_engine::stroke::Tool;

use super::{ThumbnailCache, ThumbnailKey, next_missing, open_studio, render};
use crate::state::test_support::{
    PAINT_ROW_Y, brush_totals, painting_state, present_one_frame, row_samples, stroke,
};

#[test]
fn a_key_depends_on_head_hair_and_size_but_not_the_name() {
    let mut brush = factory_brushes()[0].clone();
    let key = ThumbnailKey::of(&brush);
    brush.name = "Renamed".into();
    assert_eq!(ThumbnailKey::of(&brush), key);
    brush.size_px += 1.0;
    assert_ne!(ThumbnailKey::of(&brush), key);
}

#[test]
fn missing_lists_each_uncached_brush_once_in_order() {
    let brushes = factory_brushes();
    let mut cache = ThumbnailCache::default();
    cache.insert(ThumbnailKey::of(&brushes[1]), Vec::<u8>::new());
    let missing = cache.missing(&brushes);
    assert_eq!(missing.first(), Some(&0));
    assert!(!missing.contains(&1));
    assert_eq!(missing.len(), brushes.len() - 1);
}

#[test]
fn twins_are_missing_only_once() {
    let mut brushes = factory_brushes();
    let mut twin = brushes[0].clone();
    twin.name = "Twin".into();
    brushes.push(twin);
    let cache = ThumbnailCache::<Vec<u8>>::default();
    let missing = cache.missing(&brushes);
    assert_eq!(missing.len(), brushes.len() - 1);
    assert!(!missing.contains(&(brushes.len() - 1)));
}

#[test]
fn a_key_changes_with_the_head_and_the_hair() {
    let brush = factory_brushes()[0].clone();
    let key = ThumbnailKey::of(&brush);
    let mut thicker = brush.clone();
    thicker.head.density_per_px += 0.5;
    let mut wetter = brush;
    wetter.hair.wicking += 0.01;
    assert_ne!(ThumbnailKey::of(&thicker), key);
    assert_ne!(ThumbnailKey::of(&wetter), key);
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
