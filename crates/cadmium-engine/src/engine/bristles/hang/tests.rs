use super::*;
use crate::engine::bristles::layout::{LayoutRequest, strands};
use crate::params::SimParams;

const RADIUS_PX: f32 = 20.0;
const UPRIGHT: f32 = std::f32::consts::FRAC_PI_2;
const LEANING: f32 = 50.0 * std::f32::consts::PI / 180.0;

fn rests(shape: BrushShape) -> Vec<Strand> {
    strands(&LayoutRequest {
        shape,
        ferrule_radius_px: RADIUS_PX,
        length_ratio: 1.5,
        density_per_px: SimParams::default().get("bristle_density"),
        seed: 7,
    })
}

fn tips(shape: BrushShape, altitude: f32, azimuth: f32, orientation: f32) -> Vec<[f32; 2]> {
    let rests = rests(shape);
    resting_tips(
        &RestingBrush {
            strands: &rests,
            shape,
            ferrule_radius_px: RADIUS_PX,
        },
        &PenLean {
            altitude,
            azimuth,
            orientation,
        },
    )
    .iter()
    .map(|tip| tip.at_px)
    .collect()
}

fn mean(points: &[[f32; 2]]) -> [f32; 2] {
    let count = points.len() as f32;
    let sum = points.iter().fold([0.0, 0.0], |sum, point| {
        [sum[0] + point[0], sum[1] + point[1]]
    });
    [sum[0] / count, sum[1] / count]
}

fn span(points: &[[f32; 2]], axis: usize) -> f32 {
    let values = points.iter().map(|point| point[axis]);
    values.clone().fold(f32::MIN, f32::max) - values.fold(f32::MAX, f32::min)
}

#[test]
fn an_upright_round_brush_rests_on_a_disc_around_the_nib() {
    let points = tips(BrushShape::Round, UPRIGHT, 0.0, 0.0);
    let [x, y] = mean(&points);
    assert!(x.hypot(y) < 1.0, "{x}, {y}");
    let farthest = points
        .iter()
        .map(|point| point[0].hypot(point[1]))
        .fold(0.0, f32::max);
    assert!((farthest - RADIUS_PX).abs() < 0.2 * RADIUS_PX, "{farthest}");
}

#[test]
fn a_leaning_brush_still_rests_around_the_nib() {
    for shape in [BrushShape::Round, BrushShape::Flat, BrushShape::Fan] {
        let points = tips(shape, LEANING, 0.0, 0.0);
        let [x, y] = mean(&points);
        assert!(x.hypot(y) < 0.15 * RADIUS_PX, "{shape:?}: {x}, {y}");
    }
}

#[test]
fn a_flat_brush_rests_on_a_bar_across_its_orientation() {
    let points = tips(BrushShape::Flat, UPRIGHT, 0.0, UPRIGHT);
    let (wide, thin) = (span(&points, 0), span(&points, 1));
    assert!(wide > 3.0 * thin, "{wide} x {thin}");
}

#[test]
fn a_fan_rests_wider_and_thinner_than_a_flat_brush() {
    let flat = tips(BrushShape::Flat, UPRIGHT, 0.0, UPRIGHT);
    let fan = tips(BrushShape::Fan, UPRIGHT, 0.0, UPRIGHT);
    assert!(span(&fan, 0) > span(&flat, 0));
    assert!(span(&fan, 1) < span(&flat, 1));
}
