use super::handle::{HandleGauge, HandlePose};
use super::layout::Strand;
use super::rig::ROUND_ROOT_ORIENTATION;
use super::state::{BRISTLE_FLARE_SHARE, FAN_REST_SPLAY_SHARE};
use crate::brush::BrushShape;
use crate::stroke::Dab;

const MIN_HANGING_DROP: f32 = 0.05;
const MAX_RUN_LENGTHS: f32 = 2.0;

pub(crate) struct RestingBrush<'a> {
    pub(crate) strands: &'a [Strand],
    pub(crate) shape: BrushShape,
    pub(crate) ferrule_radius_px: f32,
}

pub(crate) struct PenLean {
    pub(crate) altitude: f32,
    pub(crate) azimuth: f32,
    pub(crate) orientation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RestingTip {
    pub(crate) at_px: [f32; 2],
    pub(crate) radius_px: f32,
}

/// Where each bristle meets the canvas when the brush is set down at the nib,
/// relative to the nib: the hanging pose the bristle shaders start a stroke
/// from, with each bristle followed down to the canvas.
pub(crate) fn resting_tips(brush: &RestingBrush<'_>, lean: &PenLean) -> Vec<RestingTip> {
    let pose = resting_pose(brush, lean);
    let orientation = root_orientation(brush.shape, lean.orientation);
    brush
        .strands
        .iter()
        .map(|strand| landing(strand, &pose, orientation, brush.ferrule_radius_px))
        .collect()
}

fn resting_pose(brush: &RestingBrush<'_>, lean: &PenLean) -> HandlePose {
    let reach_px = brush
        .strands
        .iter()
        .map(|strand| strand.length_px)
        .fold(0.0, f32::max);
    let gauge = HandleGauge {
        reach_px,
        ferrule_radius_px: brush.ferrule_radius_px,
        press_depth: 0.0,
    };
    gauge.lifted(&Dab {
        x: 0.0,
        y: 0.0,
        radius: brush.ferrule_radius_px,
        pressure: 0.0,
        altitude: lean.altitude,
        azimuth: lean.azimuth,
        direction: 0.0,
        speed: 0.0,
        time_ms: 0.0,
    })
}

fn root_orientation(shape: BrushShape, orientation: f32) -> [f32; 2] {
    if shape == BrushShape::Round {
        return ROUND_ROOT_ORIENTATION;
    }
    let (sin, cos) = orientation.sin_cos();
    [cos, sin]
}

fn landing(
    strand: &Strand,
    pose: &HandlePose,
    orientation: [f32; 2],
    ferrule_radius_px: f32,
) -> RestingTip {
    let offset = spun(strand.root_px, orientation);
    let root = add(
        pose.ferrule_px,
        tilted([offset[0], offset[1], 0.0], pose.axis),
    );
    let lean = spun(base_lean(strand, ferrule_radius_px), orientation);
    let hanging = tilted(normalized([lean[0], lean[1], -1.0]), pose.axis);
    let run_px =
        (root[2] / (-hanging[2]).max(MIN_HANGING_DROP)).min(MAX_RUN_LENGTHS * strand.length_px);
    RestingTip {
        at_px: [root[0] + hanging[0] * run_px, root[1] + hanging[1] * run_px],
        radius_px: strand.radius_px,
    }
}

fn base_lean(strand: &Strand, ferrule_radius_px: f32) -> [f32; 2] {
    let flare = BRISTLE_FLARE_SHARE / ferrule_radius_px.max(1.0);
    [0, 1].map(|axis| strand.splay[axis] * FAN_REST_SPLAY_SHARE + strand.root_px[axis] * flare)
}

fn spun(local: [f32; 2], orientation: [f32; 2]) -> [f32; 2] {
    [
        -local[0] * orientation[1] - local[1] * orientation[0],
        local[0] * orientation[0] - local[1] * orientation[1],
    ]
}

fn tilted(local: [f32; 3], axis: [f32; 3]) -> [f32; 3] {
    let lean = 1.0 / (1.0 + axis[2]);
    let across = [
        1.0 - axis[0] * axis[0] * lean,
        -axis[0] * axis[1] * lean,
        -axis[0],
    ];
    let along = [
        -axis[0] * axis[1] * lean,
        1.0 - axis[1] * axis[1] * lean,
        -axis[1],
    ];
    [0, 1, 2].map(|i| across[i] * local[0] + along[i] * local[1] + axis[i] * local[2])
}

fn normalized(vector: [f32; 3]) -> [f32; 3] {
    let length = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    vector.map(|value| value / length)
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[cfg(test)]
mod tests;
