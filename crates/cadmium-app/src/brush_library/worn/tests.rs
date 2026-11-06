use cadmium_engine::brush::{BrushSettings, factory_brushes};

use super::{index_after_delete, worn_brush};

#[test]
fn the_default_settings_wear_the_first_factory_brush() {
    let settings = BrushSettings::default();
    assert_eq!(worn_brush(&factory_brushes(), None, &settings), Some(0));
}

#[test]
fn tuned_settings_wear_no_factory_brush() {
    let mut settings = BrushSettings::default();
    settings.hair.clumping = 0.123;
    assert_eq!(worn_brush(&factory_brushes(), Some(0), &settings), None);
}

#[test]
fn the_remembered_brush_wins_over_an_earlier_twin() {
    let mut brushes = factory_brushes();
    brushes.push(brushes[0].clone());
    let twin = brushes.len() - 1;
    let settings = BrushSettings::default();
    assert_eq!(worn_brush(&brushes, Some(twin), &settings), Some(twin));
    assert_eq!(worn_brush(&brushes, None, &settings), Some(0));
}

#[test]
fn a_remembered_brush_that_no_longer_matches_is_ignored() {
    let settings = BrushSettings::default();
    assert_eq!(worn_brush(&factory_brushes(), Some(3), &settings), Some(0));
    assert_eq!(
        worn_brush(&factory_brushes(), Some(999), &settings),
        Some(0)
    );
}

#[test]
fn deleting_a_brush_shifts_the_later_ones_down() {
    assert_eq!(index_after_delete(Some(20), 18), Some(19));
    assert_eq!(index_after_delete(Some(17), 18), Some(17));
    assert_eq!(index_after_delete(Some(18), 18), None);
    assert_eq!(index_after_delete(None, 18), None);
}
