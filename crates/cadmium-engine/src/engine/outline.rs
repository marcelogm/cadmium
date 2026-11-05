use super::bristles::{
    LayoutKey, LayoutRequest, PenLean, RestingBrush, RestingTip, Strand, length_ratio,
    resting_tips, strands,
};
use super::footprint::{Footprint, blade_orientation};
use super::{BRISTLE_LAYOUT_SEED, MAX_TIP_RADIUS, MIN_DAB_RADIUS, WetOilEngine};
use crate::brush::BrushSettings;
use crate::stroke::{Dab, StrokeSample, pen_angles};

const TIP_RIM_POINTS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub enum BrushOutline {
    Ellipse {
        semi_axes: [f32; 2],
        orientation: f32,
    },
    /// The convex outline of where the bristles meet the canvas, in canvas
    /// pixels around the nib.
    Bristles(Vec<[f32; 2]>),
}

#[derive(Default)]
pub(super) struct OutlineStrands {
    cached: Option<(LayoutKey, Vec<Strand>)>,
}

impl OutlineStrands {
    fn of(&mut self, request: &LayoutRequest) -> &[Strand] {
        let key = LayoutKey::of(request);
        if self.cached.as_ref().map(|(cached, _)| *cached) != Some(key) {
            self.cached = Some((key, strands(request)));
        }
        &self.cached.as_ref().expect("filled above").1
    }
}

impl WetOilEngine {
    pub fn brush_outline(
        &self,
        brush: &BrushSettings,
        sample: &StrokeSample,
    ) -> Option<BrushOutline> {
        let profile = brush.tool.profile();
        let angles = pen_angles(sample.tilt_x_deg, sample.tilt_y_deg);
        let dab = Dab {
            x: 0.0,
            y: 0.0,
            radius: brush.geometry.max_diameter_px / 2.0,
            pressure: 1.0,
            altitude: angles.altitude_rad,
            azimuth: angles.azimuth_rad,
            direction: 0.0,
            speed: 0.0,
            time_ms: 0.0,
        };
        if profile.has_bristle_footprint() {
            return Some(self.bristle_outline(brush, &dab));
        }
        if !profile.has_disc_footprint() {
            return None;
        }
        let footprint = Footprint::of(&dab, brush, &self.params, self.tip_radius);
        Some(BrushOutline::Ellipse {
            semi_axes: footprint.semi_axes(),
            orientation: footprint.orientation,
        })
    }

    fn bristle_outline(&self, brush: &BrushSettings, dab: &Dab) -> BrushOutline {
        let radius_px = dab.radius.clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
        let request = LayoutRequest {
            shape: brush.shape,
            ferrule_radius_px: radius_px,
            length_ratio: length_ratio(brush.shape, &self.params),
            density_per_px: self.params.get("bristle_density"),
            seed: BRISTLE_LAYOUT_SEED,
        };
        let mut cache = self.outline_strands.borrow_mut();
        let tips = resting_tips(
            &RestingBrush {
                strands: cache.of(&request),
                shape: brush.shape,
                ferrule_radius_px: radius_px,
            },
            &PenLean {
                altitude: dab.altitude,
                azimuth: dab.azimuth,
                orientation: blade_orientation(brush.shape, dab.altitude, dab.azimuth),
            },
        );
        let rims = tips.iter().flat_map(tip_rim);
        BrushOutline::Bristles(convex_hull(rims.collect()))
    }
}

fn tip_rim(tip: &RestingTip) -> impl Iterator<Item = [f32; 2]> + '_ {
    (0..TIP_RIM_POINTS).map(|step| {
        let angle = std::f32::consts::TAU * step as f32 / TIP_RIM_POINTS as f32;
        [
            tip.at_px[0] + tip.radius_px * angle.cos(),
            tip.at_px[1] + tip.radius_px * angle.sin(),
        ]
    })
}

fn cross(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn half_hull(points: impl Iterator<Item = [f32; 2]>) -> Vec<[f32; 2]> {
    let mut hull: Vec<[f32; 2]> = Vec::new();
    for point in points {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0 {
            hull.pop();
        }
        hull.push(point);
    }
    hull.pop();
    hull
}

fn convex_hull(mut points: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let mut hull = half_hull(points.iter().copied());
    hull.extend(half_hull(points.iter().rev().copied()));
    hull
}

#[cfg(test)]
mod tests;
