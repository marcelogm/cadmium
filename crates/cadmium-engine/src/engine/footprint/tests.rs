use super::{BLADE_TURN_SHARE, Footprint, MIXING_KNIFE_HALF_WIDTH_RATIO, turned_blade};
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
    Footprint::of(&dab, &brush, DIAMETER_PX / 2.0, None)
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

const HALF_LENGTH_PX: f32 = 40.0;
const ANGLE_TOLERANCE: f32 = 1e-5;

#[test]
fn a_stroke_s_first_blade_lies_across_its_heading() {
    assert_eq!(
        turned_blade(None, DIRECTION, 1.0, HALF_LENGTH_PX),
        DIRECTION
    );
}

#[test]
fn a_blade_turns_only_as_far_as_its_centre_travels() {
    let quarter = std::f32::consts::FRAC_PI_2;
    let turned = turned_blade(Some(0.0), quarter * 0.9, 2.0, HALF_LENGTH_PX);
    let most = BLADE_TURN_SHARE * 2.0 / HALF_LENGTH_PX;
    assert!((turned - most).abs() < ANGLE_TOLERANCE, "{turned}");
    assert_eq!(
        turned_blade(Some(0.0), quarter * 0.9, 0.0, HALF_LENGTH_PX),
        0.0
    );
}

#[test]
fn a_blade_settles_on_a_heading_within_reach() {
    let turned = turned_blade(Some(0.0), 0.01, 2.0, HALF_LENGTH_PX);
    assert!((turned - 0.01).abs() < ANGLE_TOLERANCE, "{turned}");
}

#[test]
fn heading_back_leaves_the_blade_as_it_lies() {
    let back = std::f32::consts::PI;
    let turned = turned_blade(Some(0.0), back, 2.0, HALF_LENGTH_PX);
    assert!(turned.abs() < ANGLE_TOLERANCE, "{turned}");
}
