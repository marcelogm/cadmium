use super::{
    BLADE_COVERAGE_MARGIN_PX, BLADE_SPILL_BAND_PX, MAX_TIP_RADIUS, MIN_DAB_RADIUS, WetOilEngine,
};
use crate::brush::{BrushSettings, BrushShape};
use crate::params::SimParams;
use crate::stroke::{Dab, StrokeSample, pen_angles};

const FLAT_THICKNESS_SHARE: f32 = 0.25;
const FAN_THICKNESS_SHARE: f32 = 0.12;
const UPRIGHT_TILT_RAD: f32 = 5.0 * std::f32::consts::PI / 180.0;
const LEANING_TILT_RAD: f32 = 20.0 * std::f32::consts::PI / 180.0;
const UPRIGHT_ORIENTATION: f32 = std::f32::consts::FRAC_PI_2;
const KNIFE_MIN_HALF_WIDTH_PX: f32 = 1.5;
const KNIFE_HALF_WIDTH_RATIO: f32 = 0.12;

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
}

impl Footprint {
    pub(super) fn of(dab: &Dab, brush: &BrushSettings, params: &SimParams) -> Footprint {
        if brush.tool.profile().has_blade_footprint() {
            let radius =
                (brush.geometry.max_diameter_px / 2.0).clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
            return Footprint {
                radius,
                elongation: knife_half_width(radius) / radius,
                orientation: dab.direction,
                rectangular: true,
                lateral_reach: BLADE_SPILL_BAND_PX + BLADE_COVERAGE_MARGIN_PX,
            };
        }
        let radius = dab.radius.clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
        let tilt = 1.0 - dab.altitude.sin().clamp(0.0, 1.0);
        let elongation = (shape_thickness(brush.shape)
            * (1.0 + params.get("tilt_elongation") * tilt))
            .min(MAX_TIP_RADIUS / radius);
        let orientation = blade_orientation(brush.shape, dab.altitude, dab.azimuth);
        Footprint {
            radius,
            elongation,
            orientation,
            rectangular: false,
            lateral_reach: 0.0,
        }
    }

    pub(super) fn semi_axes(&self) -> [f32; 2] {
        [self.radius * self.elongation, self.radius]
    }

    pub(super) fn canvas_half_extents(&self) -> [f32; 2] {
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

fn shape_thickness(shape: BrushShape) -> f32 {
    match shape {
        BrushShape::Round => 1.0,
        BrushShape::Flat => FLAT_THICKNESS_SHARE,
        BrushShape::Fan => FAN_THICKNESS_SHARE,
    }
}

fn blade_orientation(shape: BrushShape, altitude: f32, azimuth: f32) -> f32 {
    if shape == BrushShape::Round {
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

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn knife_half_width(radius: f32) -> f32 {
    (KNIFE_HALF_WIDTH_RATIO * radius).max(KNIFE_MIN_HALF_WIDTH_PX)
}

impl WetOilEngine {
    pub fn brush_outline(
        &self,
        brush: &BrushSettings,
        sample: &StrokeSample,
    ) -> Option<BrushOutline> {
        if !brush.tool.profile().has_ellipse_footprint() {
            return None;
        }
        let angles = pen_angles(sample.tilt_x_deg, sample.tilt_y_deg);
        let dab = Dab {
            x: sample.x as f32,
            y: sample.y as f32,
            radius: brush.geometry.max_diameter_px / 2.0,
            pressure: 1.0,
            altitude: angles.altitude_rad,
            azimuth: angles.azimuth_rad,
            direction: 0.0,
            speed: 0.0,
        };
        let footprint = Footprint::of(&dab, brush, &self.params);
        Some(BrushOutline {
            semi_axes: footprint.semi_axes(),
            orientation: footprint.orientation,
        })
    }
}
