use crate::canvas::{CanvasSpec, weave_heights};

use super::THICKNESS_BYTES;

pub(super) fn dry_relief(spec: &CanvasSpec, dry_height: &[u8]) -> Vec<u8> {
    let weave = weave_heights(spec.weave, spec.width, spec.height);
    combine_thicknesses(dry_height, &weave, |height, weave| height - weave)
}

pub(super) fn dry_height_over_weave(spec: &CanvasSpec, relief: &[u8]) -> Vec<u8> {
    let weave = weave_heights(spec.weave, spec.width, spec.height);
    combine_thicknesses(relief, &weave, |relief, weave| relief + weave)
}

fn combine_thicknesses(raw: &[u8], weave: &[f32], combine: impl Fn(f32, f32) -> f32) -> Vec<u8> {
    raw.as_chunks::<THICKNESS_BYTES>()
        .0
        .iter()
        .zip(weave)
        .flat_map(|(texel, weave)| combine(f32::from_le_bytes(*texel), *weave).to_le_bytes())
        .collect()
}
