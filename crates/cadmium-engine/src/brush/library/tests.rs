use super::BrushSpec;
use super::{default_brush, factory_brushes};
use crate::brush::{HairSpec, HeadSpec};

#[test]
fn the_default_brush_is_todays_round_synthetic_at_forty_pixels() {
    let brush = default_brush();
    assert_eq!(brush.name, "Round · Synthetic");
    assert_eq!(brush.head, HeadSpec::ROUND);
    assert_eq!(brush.hair, HairSpec::default());
    assert_eq!(brush.size_px, 40.0);
}

#[test]
fn the_factory_has_fifteen_brushes_with_unique_names() {
    let brushes = factory_brushes();
    assert_eq!(brushes.len(), 15);
    let mut names: Vec<&str> = brushes.iter().map(|b| b.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 15);
}

#[test]
fn every_factory_brush_is_in_range_and_fits_the_size_slider() {
    for brush in factory_brushes() {
        assert_eq!(brush.head.clamped(), brush.head, "{}", brush.name);
        assert_eq!(brush.hair.clamped(), brush.hair, "{}", brush.name);
        assert!(
            (BrushSpec::SIZE_RANGE_PX[0]..=BrushSpec::SIZE_RANGE_PX[1]).contains(&brush.size_px),
            "{}",
            brush.name
        );
    }
}

#[test]
fn every_factory_head_and_hair_is_on_some_brush() {
    let brushes = factory_brushes();
    for head in [
        HeadSpec::ROUND,
        HeadSpec::FLAT,
        HeadSpec::BRIGHT,
        HeadSpec::FILBERT,
        HeadSpec::FAN,
        HeadSpec::RIGGER,
        HeadSpec::ANGULAR,
        HeadSpec::DAGGER,
        HeadSpec::MOP,
    ] {
        assert!(brushes.iter().any(|b| b.head == head), "{head:?}");
    }
    for hair in [
        HairSpec::SYNTHETIC,
        HairSpec::HOG,
        HairSpec::SABLE,
        HairSpec::MONGOOSE,
        HairSpec::SQUIRREL,
    ] {
        assert!(brushes.iter().any(|b| b.hair == hair), "{hair:?}");
    }
}
