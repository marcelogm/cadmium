use cadmium_engine::brush::factory_brushes;

use super::{ThumbnailCache, ThumbnailKey};

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
