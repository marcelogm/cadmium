use super::layout::{
    BRISTLE_JITTER_SHARE, BRISTLES_PER_CLUSTER, BUNDLE_COVERING_GAIN, FULL_BUNDLE_GAIN_RADIUS_PX,
    MAX_BRISTLES, MIN_BRISTLES,
};
use crate::brush::BrushShape;
use crate::engine::footprint::{FAN_THICKNESS_SHARE, FLAT_THICKNESS_SHARE};

const FAN_SPREAD_DEG: f32 = 50.0;
const FAN_ARC_RADIUS_SHARE: f32 = 0.6;
const GOLDEN_ANGLE_RAD: f32 = 2.399_963_2;
const SPIRAL_JITTER_SHARE: f32 = 0.3;
const LATTICE_STEP: [f32; 2] = [0.754_877_7, 0.569_840_3];
const LATTICE_JITTER_SHARE: f32 = 0.5;

pub(super) struct Root {
    pub(super) position_px: [f32; 2],
    pub(super) splay: [f32; 2],
    pub(super) lane_direction: [f32; 2],
}

pub(super) struct SplitMix(u64);

impl SplitMix {
    pub(super) fn new(seed: u64) -> SplitMix {
        SplitMix(seed)
    }

    fn next_bits(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    pub(super) fn unit(&mut self) -> f32 {
        (self.next_bits() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub(super) fn jitter(&mut self) -> f32 {
        (self.unit() * 2.0 - 1.0) * BRISTLE_JITTER_SHARE
    }
}

pub(super) fn bristle_count(radius_px: f32, density_per_px: f32) -> usize {
    ((2.0 * radius_px * density_per_px).round() as usize).clamp(MIN_BRISTLES, MAX_BRISTLES)
}

pub(super) fn capped_count_onset_radius_px(density_per_px: f32) -> f32 {
    MAX_BRISTLES as f32 / (2.0 * density_per_px.max(f32::MIN_POSITIVE))
}

pub(super) fn bundle_gain(radius_px: f32, density_per_px: f32) -> f32 {
    let onset_px = capped_count_onset_radius_px(density_per_px);
    if onset_px >= FULL_BUNDLE_GAIN_RADIUS_PX {
        return 1.0;
    }
    let progress =
        ((radius_px - onset_px) / (FULL_BUNDLE_GAIN_RADIUS_PX - onset_px)).clamp(0.0, 1.0);
    let eased = progress * progress * (3.0 - 2.0 * progress);
    1.0 + (BUNDLE_COVERING_GAIN - 1.0) * eased
}

pub(super) fn covered_area_px2(shape: BrushShape, radius_px: f32) -> f32 {
    let diameter_px = 2.0 * radius_px;
    match shape {
        BrushShape::Round => std::f32::consts::PI * radius_px * radius_px,
        BrushShape::Flat => diameter_px * diameter_px * FLAT_THICKNESS_SHARE,
        BrushShape::Fan => {
            let arc_px = FAN_ARC_RADIUS_SHARE * radius_px * 2.0 * FAN_SPREAD_DEG.to_radians();
            arc_px * diameter_px * FAN_THICKNESS_SHARE
        }
    }
}

pub(super) fn place_roots(
    shape: BrushShape,
    radius_px: f32,
    count: usize,
    random: &mut SplitMix,
) -> Vec<Root> {
    (0..count)
        .map(|index| match shape {
            BrushShape::Round => spiral_root(index, count, radius_px, random),
            BrushShape::Flat => flat_root(index, count, radius_px, random),
            BrushShape::Fan => fan_root(index, count, radius_px, random),
        })
        .collect()
}

fn spiral_root(index: usize, count: usize, radius_px: f32, random: &mut SplitMix) -> Root {
    let spacing_px = radius_px / (count as f32).sqrt();
    let ideal_px = radius_px * ((index as f32 + 0.5) / count as f32).sqrt();
    let jitter_px = (random.unit() - 0.5) * SPIRAL_JITTER_SHARE * spacing_px;
    let distance_px = (ideal_px + jitter_px).clamp(0.0, radius_px);
    let angle = index as f32 * GOLDEN_ANGLE_RAD + (random.unit() - 0.5) * SPIRAL_JITTER_SHARE;
    let outward = [angle.cos(), angle.sin()];
    Root {
        position_px: [outward[0] * distance_px, outward[1] * distance_px],
        splay: [0.0, 0.0],
        lane_direction: outward,
    }
}

fn lattice_point(index: usize, count: usize, random: &mut SplitMix) -> [f32; 2] {
    let wobble = LATTICE_JITTER_SHARE / (count as f32).sqrt();
    let coordinate = |axis: usize, random: &mut SplitMix| {
        let ideal = 0.5 + index as f32 * LATTICE_STEP[axis];
        (ideal + (random.unit() - 0.5) * wobble).rem_euclid(1.0)
    };
    [coordinate(0, random), coordinate(1, random)]
}

fn flat_root(index: usize, count: usize, radius_px: f32, random: &mut SplitMix) -> Root {
    let [across, thick] = lattice_point(index, count, random);
    let mut position_px = [
        (2.0 * across - 1.0) * radius_px,
        (2.0 * thick - 1.0) * radius_px * FLAT_THICKNESS_SHARE,
    ];
    let reach_px = position_px[0].hypot(position_px[1]);
    if reach_px > radius_px {
        let shrink = radius_px / reach_px;
        position_px = [position_px[0] * shrink, position_px[1] * shrink];
    }
    let side = if position_px[1] < 0.0 { -1.0 } else { 1.0 };
    Root {
        position_px,
        splay: [0.0, 0.0],
        lane_direction: [0.0, side],
    }
}

fn fan_root(index: usize, count: usize, radius_px: f32, random: &mut SplitMix) -> Root {
    let [across, thick] = lattice_point(index, count, random);
    let angle = (2.0 * across - 1.0) * FAN_SPREAD_DEG.to_radians();
    let band_px = 2.0 * radius_px * FAN_THICKNESS_SHARE;
    let distance_px = FAN_ARC_RADIUS_SHARE * radius_px + (thick - 0.5) * band_px;
    let outward = [angle.sin(), angle.cos()];
    let spread = FAN_SPREAD_DEG.to_radians();
    let arc_centroid_px = FAN_ARC_RADIUS_SHARE * radius_px * spread.sin() / spread;
    Root {
        position_px: [
            outward[0] * distance_px,
            outward[1] * distance_px - arc_centroid_px,
        ],
        splay: [outward[0], 0.0],
        lane_direction: outward,
    }
}

pub(super) fn assign_clusters(roots: &[Root]) -> Vec<u32> {
    let clusters = (roots.len() / BRISTLES_PER_CLUSTER).max(1);
    let seeds: Vec<[f32; 2]> = (0..clusters)
        .map(|cluster| roots[cluster * roots.len() / clusters].position_px)
        .collect();
    roots
        .iter()
        .map(|root| nearest_seed(&seeds, root.position_px) as u32)
        .collect()
}

fn nearest_seed(seeds: &[[f32; 2]], point: [f32; 2]) -> usize {
    let distance = |seed: &[f32; 2]| (seed[0] - point[0]).powi(2) + (seed[1] - point[1]).powi(2);
    seeds
        .iter()
        .enumerate()
        .min_by(|a, b| distance(a.1).total_cmp(&distance(b.1)))
        .map_or(0, |(index, _)| index)
}
