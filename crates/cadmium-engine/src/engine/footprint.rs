use super::bristles::{ReachRequest, reach_bound_px};
use super::{
    BLADE_COVERAGE_MARGIN_PX, BLADE_SPILL_BAND_PX, MAX_TIP_RADIUS, MIN_DAB_RADIUS, WetOilEngine,
};
use crate::brush::BrushSettings;
use crate::stroke::{Dab, PenAngles, StrokeSample, Tool, pen_angles};

const UPRIGHT_TILT_RAD: f32 = 5.0 * std::f32::consts::PI / 180.0;
const LEANING_TILT_RAD: f32 = 20.0 * std::f32::consts::PI / 180.0;
const UPRIGHT_ORIENTATION: f32 = std::f32::consts::FRAC_PI_2;
const KNIFE_MIN_HALF_WIDTH_PX: f32 = 1.5;
const KNIFE_HALF_WIDTH_RATIO: f32 = 0.12;
const MIXING_KNIFE_HALF_WIDTH_RATIO: f32 = 0.35;
const ELLIPSE_TILT_ELONGATION: f32 = 1.5;
const BRISTLE_BOUND_MARGIN_PX: f32 = 2.0;
/// How far a blade turns per pixel its centre travels, as a share of the turn that would
/// carry its ends that pixel too: its ends then move at most half again as far as its
/// centre, so turning never opens gaps between its dabs.
const BLADE_TURN_SHARE: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushOutline {
    pub semi_axes: [f32; 2],
    pub orientation: f32,
}

pub(super) struct Footprint {
    pub(super) radius: f32,
    pub(super) elongation: f32,
    pub(super) orientation: f32,
    rectangular: bool,
    lateral_reach: f32,
    bristle_reach_px: Option<f32>,
}

impl Footprint {
    /// The dab's footprint; a blade lies at `blade_orientation` when given, else across the
    /// dab's heading.
    pub(super) fn of(
        dab: &Dab,
        brush: &BrushSettings,
        ferrule_radius_px: f32,
        blade_orientation: Option<f32>,
    ) -> Footprint {
        let profile = brush.tool.profile();
        if profile.has_blade_footprint() {
            return Footprint::of_blade(dab, brush, blade_orientation);
        }
        if profile.has_bristle_footprint() {
            return Footprint::of_bristles(dab, brush, ferrule_radius_px);
        }
        Footprint::of_ellipse(dab, brush)
    }

    fn of_blade(dab: &Dab, brush: &BrushSettings, orientation: Option<f32>) -> Footprint {
        let radius = blade_half_length(brush);
        Footprint {
            radius,
            elongation: blade_half_width(brush.tool, radius) / radius,
            orientation: orientation.unwrap_or(dab.direction),
            rectangular: true,
            lateral_reach: BLADE_SPILL_BAND_PX + BLADE_COVERAGE_MARGIN_PX,
            bristle_reach_px: None,
        }
    }

    fn of_bristles(dab: &Dab, brush: &BrushSettings, ferrule_radius_px: f32) -> Footprint {
        let radius = ferrule_radius_px.clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
        let head = brush.head.clamped();
        let reach_px = reach_bound_px(&ReachRequest {
            ferrule_radius_px: radius,
            head,
        });
        Footprint {
            radius,
            elongation: 1.0,
            orientation: blade_orientation(head.is_round(), dab.altitude, dab.azimuth),
            rectangular: false,
            lateral_reach: 0.0,
            bristle_reach_px: Some(reach_px + BRISTLE_BOUND_MARGIN_PX),
        }
    }

    fn of_ellipse(dab: &Dab, brush: &BrushSettings) -> Footprint {
        let radius = dab.radius.clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
        let head = brush.head.clamped();
        let tilt = 1.0 - dab.altitude.sin().clamp(0.0, 1.0);
        let elongation =
            (head.thickness * (1.0 + ELLIPSE_TILT_ELONGATION * tilt)).min(MAX_TIP_RADIUS / radius);
        Footprint {
            radius,
            elongation,
            orientation: blade_orientation(head.is_round(), dab.altitude, dab.azimuth),
            rectangular: false,
            lateral_reach: 0.0,
            bristle_reach_px: None,
        }
    }

    pub(super) fn semi_axes(&self) -> [f32; 2] {
        [self.radius * self.elongation, self.radius]
    }

    pub(super) fn canvas_half_extents(&self) -> [f32; 2] {
        if let Some(reach_px) = self.bristle_reach_px {
            return [reach_px; 2];
        }
        let [along, across] = self.semi_axes();
        let (sin, cos) = self.orientation.sin_cos();
        if self.rectangular {
            let along = along + BLADE_COVERAGE_MARGIN_PX;
            let across = across + self.lateral_reach;
            return [
                along * cos.abs() + across * sin.abs(),
                along * sin.abs() + across * cos.abs(),
            ];
        }
        [
            (along * along * cos * cos + across * across * sin * sin).sqrt(),
            (along * along * sin * sin + across * across * cos * cos).sqrt(),
        ]
    }
}

fn blade_orientation(round: bool, altitude: f32, azimuth: f32) -> f32 {
    if round {
        return azimuth;
    }
    let tilt = std::f32::consts::FRAC_PI_2 - altitude;
    let lean = smoothstep(UPRIGHT_TILT_RAD, LEANING_TILT_RAD, tilt);
    let doubled = [
        lean * (2.0 * azimuth).cos() + (1.0 - lean) * (2.0 * UPRIGHT_ORIENTATION).cos(),
        lean * (2.0 * azimuth).sin() + (1.0 - lean) * (2.0 * UPRIGHT_ORIENTATION).sin(),
    ];
    if doubled[0].hypot(doubled[1]) <= f32::EPSILON {
        return UPRIGHT_ORIENTATION;
    }
    doubled[1].atan2(doubled[0]) / 2.0
}

/// Half the blade's length, along its edge.
pub(super) fn blade_half_length(brush: &BrushSettings) -> f32 {
    (brush.geometry.max_diameter_px / 2.0).clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS)
}

/// Where a blade at `previous` lies once its centre has travelled `step_px` heading
/// towards `heading`: it turns towards lying across the heading, but only as far as the
/// travel allows, so a hand jiggling in place or rounding a curve sweeps the blade evenly
/// instead of swinging it about. A blade is the same either way round, so heading back
/// does not turn it.
pub(super) fn turned_blade(
    previous: Option<f32>,
    heading: f32,
    step_px: f32,
    half_length_px: f32,
) -> f32 {
    let Some(previous) = previous else {
        return heading;
    };
    let half_turn = std::f32::consts::PI;
    let wanted = (heading - previous + half_turn / 2.0).rem_euclid(half_turn) - half_turn / 2.0;
    let most = BLADE_TURN_SHARE * step_px.max(0.0) / half_length_px.max(1.0);
    previous + wanted.clamp(-most, most)
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Half the blade's width along the stroke: the painting knife is a thin edge,
/// the mixing knife lays a broad face on the paint.
fn blade_half_width(tool: Tool, radius: f32) -> f32 {
    if tool == Tool::MixingKnife {
        return MIXING_KNIFE_HALF_WIDTH_RATIO * radius;
    }
    knife_half_width(radius)
}

fn knife_half_width(radius: f32) -> f32 {
    (KNIFE_HALF_WIDTH_RATIO * radius).max(KNIFE_MIN_HALF_WIDTH_PX)
}

impl WetOilEngine {
    pub fn brush_outline(
        &self,
        brush: &BrushSettings,
        sample: &StrokeSample,
    ) -> Option<BrushOutline> {
        let profile = brush.tool.profile();
        let angles = pen_angles(sample.tilt_x_deg, sample.tilt_y_deg);
        if profile.has_bristle_footprint() {
            return Some(bristle_outline(brush, angles));
        }
        if !profile.has_ellipse_footprint() {
            return None;
        }
        let dab = Dab {
            x: sample.x as f32,
            y: sample.y as f32,
            radius: brush.geometry.max_diameter_px / 2.0,
            pressure: 1.0,
            altitude: angles.altitude_rad,
            azimuth: angles.azimuth_rad,
            direction: 0.0,
            speed: 0.0,
            time_ms: 0.0,
        };
        let footprint = Footprint::of(&dab, brush, self.tip_radius, None);
        Some(BrushOutline {
            semi_axes: footprint.semi_axes(),
            orientation: footprint.orientation,
        })
    }
}

fn bristle_outline(brush: &BrushSettings, angles: PenAngles) -> BrushOutline {
    let radius = (brush.geometry.max_diameter_px / 2.0).clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
    let head = brush.head.clamped();
    BrushOutline {
        semi_axes: [radius * head.thickness, radius],
        orientation: blade_orientation(head.is_round(), angles.altitude_rad, angles.azimuth_rad),
    }
}

#[cfg(test)]
mod tests;
