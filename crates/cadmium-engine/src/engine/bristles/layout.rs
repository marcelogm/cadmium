use super::cells::{CellFrame, cell_grids};
use super::clusters::cluster_index;
use super::owners::{build_owner_map, owner_ranks};
use super::roots::{
    Root, SplitMix, assign_clusters, bristle_count, bundle_gain, covered_area_px2, place_roots,
};
use crate::brush::BrushShape;
use crate::engine::{MAX_TIP_RADIUS, MIN_DAB_RADIUS, TIP_HALF};
use bytemuck::{Pod, Zeroable};

pub(crate) const MAX_BRISTLES: usize = 512;
pub(super) const MIN_BRISTLES: usize = 3;
pub(crate) const BRISTLE_VERTICES: usize = 6;
pub(super) const MIN_BRISTLE_RADIUS_PX: f32 = 0.6;
pub(super) const BRISTLES_PER_CLUSTER: usize = 8;
pub(super) const BRISTLE_JITTER_SHARE: f32 = 0.15;
pub(super) const BUNDLE_COVERING_GAIN: f32 = 1.75;
pub(super) const FULL_BUNDLE_GAIN_RADIUS_PX: f32 = MAX_TIP_RADIUS;
const BASE_STIFFNESS: f32 = 1.0;

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug, PartialEq)]
pub(crate) struct BristleRest {
    pub(crate) root_px: [f32; 2],
    pub(crate) splay: [f32; 2],
    pub(crate) length_px: f32,
    pub(crate) stiffness: f32,
    pub(crate) radius_px: f32,
    pub(crate) cluster: u32,
    pub(crate) lane_start: [f32; 2],
    pub(crate) lane_end: [f32; 2],
}

pub(crate) struct BristleLayout {
    pub(crate) bristles: Vec<BristleRest>,
    pub(crate) owners: Vec<u32>,
    pub(crate) ranks: Vec<u32>,
    pub(crate) cell_grids: Vec<u32>,
    pub(crate) cluster_index: Vec<u32>,
}

pub(crate) struct ReachRequest {
    pub(crate) ferrule_radius_px: f32,
    pub(crate) length_ratio: f32,
    pub(crate) density_per_px: f32,
}

pub(crate) struct LayoutRequest {
    pub(crate) shape: BrushShape,
    pub(crate) ferrule_radius_px: f32,
    pub(crate) length_ratio: f32,
    pub(crate) density_per_px: f32,
    pub(crate) seed: u64,
}

pub(crate) fn layout(request: &LayoutRequest) -> BristleLayout {
    let radius_px = request
        .ferrule_radius_px
        .clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
    let mut random = SplitMix::new(request.seed);
    let count = bristle_count(radius_px, request.density_per_px).min(disc_texel_count(radius_px));
    let roots = place_roots(request.shape, radius_px, count, &mut random);
    let clusters = assign_clusters(&roots);
    let tip_roots: Vec<[f32; 2]> = roots.iter().map(tip_position).collect();
    let map = build_owner_map(&tip_roots, &roots, radius_px);
    let coverage_radius_px =
        (covered_area_px2(request.shape, radius_px) / (std::f32::consts::PI * count as f32)).sqrt()
            * bundle_gain(radius_px, request.density_per_px);
    let bristles = roots
        .iter()
        .enumerate()
        .map(|(index, root)| BristleRest {
            root_px: root.position_px,
            splay: root.splay,
            length_px: radius_px * request.length_ratio * (1.0 + random.jitter()),
            stiffness: BASE_STIFFNESS * (1.0 + random.jitter()),
            radius_px: (coverage_radius_px * (1.0 + random.jitter())).max(MIN_BRISTLE_RADIUS_PX),
            cluster: clusters[index],
            lane_start: map.lanes[index].0,
            lane_end: map.lanes[index].1,
        })
        .collect();
    BristleLayout {
        bristles,
        ranks: owner_ranks(&map.owners),
        cell_grids: cell_grids(&map.owners, &cell_frames(&roots, &map.lanes)),
        owners: map.owners,
        cluster_index: cluster_index(&clusters),
    }
}

pub(crate) fn reach_bound_px(request: &ReachRequest) -> f32 {
    let radius_px = request
        .ferrule_radius_px
        .clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
    let longest_px = radius_px * request.length_ratio * (1.0 + BRISTLE_JITTER_SHARE);
    let count = bristle_count(radius_px, request.density_per_px).min(disc_texel_count(radius_px));
    let gain = bundle_gain(radius_px, request.density_per_px);
    let widest_px = (radius_px / (count as f32).sqrt() * gain * (1.0 + BRISTLE_JITTER_SHARE))
        .max(MIN_BRISTLE_RADIUS_PX);
    radius_px + longest_px + widest_px
}

fn cell_frames(roots: &[Root], lanes: &[([f32; 2], [f32; 2])]) -> Vec<CellFrame> {
    roots
        .iter()
        .zip(lanes)
        .map(|(root, lane)| CellFrame {
            direction: root.lane_direction,
            fallback_slot: tip_slot(lane.0),
        })
        .collect()
}

fn tip_slot(point: [f32; 2]) -> u32 {
    let last = (crate::engine::TIP_SIZE - 1) as f32;
    let [x, y] = point.map(|value| value.floor().clamp(0.0, last) as u32);
    y * crate::engine::TIP_SIZE + x
}

fn tip_position(root: &Root) -> [f32; 2] {
    let half = TIP_HALF as f32;
    [half + root.position_px[0], half + root.position_px[1]]
}

pub(crate) fn disc_texel_count(radius_px: f32) -> usize {
    let half = TIP_HALF as i32;
    let size = crate::engine::TIP_SIZE as i32;
    let radius_squared = radius_px * radius_px;
    (-half..size - half)
        .map(|y| row_texel_count(y, radius_squared))
        .sum()
}

fn inside_disc(x: i32, y: i32, radius_squared: f32) -> bool {
    let (x, y) = (x as f32, y as f32);
    x * x + y * y <= radius_squared
}

fn row_texel_count(y: i32, radius_squared: f32) -> usize {
    if !inside_disc(0, y, radius_squared) {
        return 0;
    }
    let half = TIP_HALF as i32;
    let size = crate::engine::TIP_SIZE as i32;
    let row_squared = (y * y) as f32;
    let mut reach = ((radius_squared - row_squared).max(0.0).sqrt() as i32).min(size);
    while reach < size && inside_disc(reach + 1, y, radius_squared) {
        reach += 1;
    }
    while !inside_disc(reach, y, radius_squared) {
        reach -= 1;
    }
    let first = (-reach).max(-half);
    let last = reach.min(size - half - 1);
    (last - first + 1) as usize
}

#[cfg(test)]
mod tests;
