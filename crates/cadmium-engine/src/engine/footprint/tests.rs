use super::{Footprint, MIXING_KNIFE_HALF_WIDTH_RATIO};
use crate::brush::BrushSettings;
use crate::stroke::{Dab, Tool};

const DIAMETER_PX: f32 = 80.0;
const DIRECTION: f32 = 0.7;
const KNIFE_ELONGATION: f32 = 0.12;
const ELONGATION_TOLERANCE: f32 = 1e-6;

fn blade_of(tool: Tool) -> Footprint {
    let mut brush = BrushSettings {
        tool,
        ..BrushSettings::default()
    };
    brush.geometry.max_diameter_px = DIAMETER_PX;
    let dab = Dab {
        x: 100.0,
        y: 100.0,
        radius: DIAMETER_PX / 2.0,
        pressure: 1.0,
        altitude: 0.6,
        azimuth: 2.0,
        direction: DIRECTION,
        speed: 1.0,
        time_ms: 0.0,
    };
    Footprint::of(&dab, &brush, DIAMETER_PX / 2.0)
}

#[test]
fn the_mixing_knife_blade_is_much_wider_across_the_stroke_than_the_knife_blade() {
    let knife = blade_of(Tool::Knife);
    let mixing = blade_of(Tool::MixingKnife);
    assert_eq!(mixing.radius, knife.radius);
    assert!((knife.elongation - KNIFE_ELONGATION).abs() < ELONGATION_TOLERANCE);
    assert!((mixing.elongation - MIXING_KNIFE_HALF_WIDTH_RATIO).abs() < ELONGATION_TOLERANCE);
    assert!(mixing.elongation > 2.0 * knife.elongation);
}

#[test]
fn both_knives_lay_their_blade_along_the_stroke() {
    for tool in [Tool::Knife, Tool::MixingKnife] {
        assert_eq!(blade_of(tool).orientation, DIRECTION, "{tool:?}");
    }
}
