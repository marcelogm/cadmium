use super::full_diameter_spacing;
use crate::stroke::{BrushGeometry, PressureCurve};

fn shrinking_geometry() -> BrushGeometry {
    BrushGeometry {
        max_diameter_px: 20.0,
        min_diameter_ratio: 0.2,
        spacing: 0.1,
        pressure_curve: PressureCurve::default(),
    }
}

#[test]
fn stroke_geometry_ignores_pressure_shrink_for_bristle_tools() {
    let geometry = full_diameter_spacing(shrinking_geometry(), true);
    assert_eq!(geometry.min_diameter_ratio, 1.0);
    assert_eq!(geometry.diameter(0.0), geometry.max_diameter_px);
}

#[test]
fn stroke_geometry_keeps_pressure_shrink_for_other_tools() {
    let geometry = full_diameter_spacing(shrinking_geometry(), false);
    assert_eq!(geometry.min_diameter_ratio, 0.2);
}
