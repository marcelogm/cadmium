//! Where a head's hair roots sit on the ferrule. Each root is laid by every section the head
//! has a share of (`HeadSpec::section_mix`) and blended, so thickness and fan slide the layout
//! smoothly. That covers the root layout only: `HeadSpec::is_round` still switches the rig's
//! and the footprint's root frame at thickness 1, which does not show because a near-round
//! head's roots are an isotropic disc either way.

use super::layout::{
    BRISTLE_JITTER_SHARE, BRISTLES_PER_CLUSTER, BUNDLE_COVERING_GAIN, FULL_BUNDLE_GAIN_RADIUS_PX,
    MAX_BRISTLES, MIN_BRISTLES,
};
use crate::brush::{HeadSpec, Section, SectionMix};

const FAN_ARC_RADIUS_SHARE: f32 = 0.6;
const GOLDEN_ANGLE_RAD: f32 = 2.399_963_2;
const SPIRAL_JITTER_SHARE: f32 = 0.3;
const LATTICE_STEP: [f32; 2] = [0.754_877_7, 0.569_840_3];
const LATTICE_JITTER_SHARE: f32 = 0.5;
/// Defensive: a fan spread, in radians, below which the arc's centroid is taken at its
/// radius instead of dividing by it. Only the arc a blend lays at no (or a vanishing) fan
/// gets here, at a share of (almost) 0.
const MIN_ARC_SPREAD_RAD: f32 = 1e-6;

pub(super) struct Root {
    pub(super) position_px: [f32; 2],
    pub(super) splay: [f32; 2],
    pub(super) lane_direction: [f32; 2],
    /// Where the dome and slant read this root from (`HeadSpec::length_share`), in ferrule
    /// radii (on the arc, x is the share across the spread in −1..1) across and through the
    /// head: `position_px / radius` on the spiral and the bar, the root's place across the
    /// spread and through the band on the arc.
    pub(super) shape: [f32; 2],
    pub(super) length_share: f32,
}

#[derive(Clone, Debug, PartialEq)]
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

/// Which root, of how many, on a ferrule of what radius.
#[derive(Clone, Copy)]
struct RootSlot {
    index: usize,
    count: usize,
    radius_px: f32,
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

/// The area the head's sections cover, each by its share.
pub(super) fn covered_area_px2(head: &HeadSpec, radius_px: f32) -> f32 {
    head.section_mix()
        .parts()
        .map(|(section, share)| share * section_area_px2(section, head, radius_px))
        .sum()
}

fn section_area_px2(section: Section, head: &HeadSpec, radius_px: f32) -> f32 {
    let diameter_px = 2.0 * radius_px;
    match section {
        Section::Round => std::f32::consts::PI * radius_px * radius_px,
        Section::Bar => diameter_px * diameter_px * head.thickness,
        Section::Arc => {
            let arc_px = FAN_ARC_RADIUS_SHARE * radius_px * 2.0 * head.fan_deg.to_radians();
            arc_px * diameter_px * head.thickness
        }
    }
}

pub(super) fn place_roots(
    head: &HeadSpec,
    radius_px: f32,
    count: usize,
    random: &mut SplitMix,
) -> Vec<Root> {
    let mix = head.section_mix();
    (0..count)
        .map(|index| {
            let slot = RootSlot {
                index,
                count,
                radius_px,
            };
            let mut root = mixed_root(head, mix, slot, random);
            root.length_share = head.length_share(root.shape);
            root
        })
        .collect()
}

/// The root each section with a share lays, blended by those shares. Every section draws the
/// same two numbers per root, so each part starts from the same draws and `random` ends where
/// one section alone would leave it: the hairs keep their jitter across a blend. A single
/// section's root is returned untouched; a blend lays all three, as the lane turns through
/// each section's.
fn mixed_root(head: &HeadSpec, mix: SectionMix, slot: RootSlot, random: &mut SplitMix) -> Root {
    let start = random.clone();
    let mut lay = |section: Section| {
        *random = start.clone();
        section_root(section, head, slot, random)
    };
    let mut parts = mix.parts();
    match (parts.next(), parts.next()) {
        (Some((section, _)), None) => lay(section),
        _ => blend(mix, [Section::Round, Section::Bar, Section::Arc].map(lay)),
    }
}

fn section_root(section: Section, head: &HeadSpec, slot: RootSlot, random: &mut SplitMix) -> Root {
    let RootSlot {
        index,
        count,
        radius_px,
    } = slot;
    let mut root = match section {
        Section::Round => spiral_root(index, count, radius_px, random),
        Section::Bar => flat_root(index, count, radius_px, head.thickness, random),
        Section::Arc => fan_root(index, count, radius_px, head, random),
    };
    let unit = unit_of(root.position_px, radius_px);
    bulge(&mut root, head, section, unit);
    root
}

fn bulge(root: &mut Root, head: &HeadSpec, section: Section, unit: [f32; 2]) {
    let amount = head.bulge(section);
    if amount <= 0.0 {
        return;
    }
    root.splay[0] += unit[0] * amount;
    if section != Section::Arc {
        root.splay[1] += unit[1] * amount;
    }
}

/// The round, bar and arc roots weighted by their shares, the lane turned between them.
fn blend(mix: SectionMix, [round, bar, arc]: [Root; 3]) -> Root {
    let shares = [mix.round, mix.bar, mix.arc];
    let parts = [&round, &bar, &arc];
    let sum = |pick: fn(&Root) -> [f32; 2]| {
        shares
            .iter()
            .zip(parts)
            .fold([0.0; 2], |total, (share, root)| {
                let value = pick(root);
                [total[0] + share * value[0], total[1] + share * value[1]]
            })
    };
    let lanes = parts.map(|root| root.lane_direction);
    Root {
        position_px: sum(|root| root.position_px),
        splay: sum(|root| root.splay),
        lane_direction: blended_lane(mix, lanes),
        shape: sum(|root| root.shape),
        length_share: 1.0,
    }
}

/// The round lane turned toward the bar's by the bar and arc shares, then on toward the
/// arc's by the arc share. A lane is a ray from its root through its cell, so its sign
/// matters; a weighted sum of near-opposite lanes would swing round at an even share, a turn
/// moves with the shares. Each section's lane moves smoothly with the controls (the arc's
/// only toward ±180° from a bar's as its share fades to 0), so the turn does too, and it is
/// the lone section's lane wherever one section is left.
fn blended_lane(mix: SectionMix, [round, bar, arc]: [[f32; 2]; 3]) -> [f32; 2] {
    let turn = (mix.bar + mix.arc) * turn_between(round, bar) + mix.arc * turn_between(bar, arc);
    let (sin, cos) = libm::sincosf(turn);
    [
        round[0] * cos - round[1] * sin,
        round[0] * sin + round[1] * cos,
    ]
}

/// The signed angle, in (−π, π], from unit direction `from` to unit direction `to`.
fn turn_between(from: [f32; 2], to: [f32; 2]) -> f32 {
    let cross = from[0] * to[1] - from[1] * to[0];
    let dot = from[0] * to[0] + from[1] * to[1];
    libm::atan2f(cross, dot)
}

fn unit_of(position_px: [f32; 2], radius_px: f32) -> [f32; 2] {
    position_px.map(|value| value / radius_px)
}

fn spiral_root(index: usize, count: usize, radius_px: f32, random: &mut SplitMix) -> Root {
    let spacing_px = radius_px / (count as f32).sqrt();
    let ideal_px = radius_px * ((index as f32 + 0.5) / count as f32).sqrt();
    let jitter_px = (random.unit() - 0.5) * SPIRAL_JITTER_SHARE * spacing_px;
    let distance_px = (ideal_px + jitter_px).clamp(0.0, radius_px);
    let angle = index as f32 * GOLDEN_ANGLE_RAD + (random.unit() - 0.5) * SPIRAL_JITTER_SHARE;
    let outward = [libm::cosf(angle), libm::sinf(angle)];
    let position_px = [outward[0] * distance_px, outward[1] * distance_px];
    Root {
        position_px,
        splay: [0.0, 0.0],
        lane_direction: outward,
        shape: unit_of(position_px, radius_px),
        length_share: 1.0,
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

fn flat_root(
    index: usize,
    count: usize,
    radius_px: f32,
    thickness: f32,
    random: &mut SplitMix,
) -> Root {
    let [across, thick] = lattice_point(index, count, random);
    let mut position_px = [
        (2.0 * across - 1.0) * radius_px,
        (2.0 * thick - 1.0) * radius_px * thickness,
    ];
    let reach_px = libm::hypotf(position_px[0], position_px[1]);
    if reach_px > radius_px {
        let shrink = radius_px / reach_px;
        position_px = [position_px[0] * shrink, position_px[1] * shrink];
    }
    let side = if position_px[1] < 0.0 { -1.0 } else { 1.0 };
    Root {
        position_px,
        splay: [0.0, 0.0],
        lane_direction: [0.0, side],
        shape: unit_of(position_px, radius_px),
        length_share: 1.0,
    }
}

/// A root on the fan's arc. Its `shape` is its place across the spread and through the band
/// — what the bar's position is before its rim pull — so the dome shortens the fan's outer
/// hairs and the slant grows them from one side of the spread to the other.
fn fan_root(
    index: usize,
    count: usize,
    radius_px: f32,
    head: &HeadSpec,
    random: &mut SplitMix,
) -> Root {
    let [across, thick] = lattice_point(index, count, random);
    let spread = head.fan_deg.to_radians();
    let angle = (2.0 * across - 1.0) * spread;
    let band_px = 2.0 * radius_px * head.thickness;
    let distance_px = FAN_ARC_RADIUS_SHARE * radius_px + (thick - 0.5) * band_px;
    let outward = [libm::sinf(angle), libm::cosf(angle)];
    let position_px = [
        outward[0] * distance_px,
        outward[1] * distance_px - arc_centroid_px(radius_px, spread),
    ];
    Root {
        position_px,
        splay: [outward[0], 0.0],
        lane_direction: outward,
        shape: [2.0 * across - 1.0, (2.0 * thick - 1.0) * head.thickness],
        length_share: 1.0,
    }
}

/// How far along the fan's axis the arc's roots sit on average: the arc is shifted back by
/// it, so the fan stays centred on the ferrule. A spread too small to divide by is straight.
fn arc_centroid_px(radius_px: f32, spread: f32) -> f32 {
    if spread > MIN_ARC_SPREAD_RAD {
        FAN_ARC_RADIUS_SHARE * radius_px * libm::sinf(spread) / spread
    } else {
        FAN_ARC_RADIUS_SHARE * radius_px
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

#[cfg(test)]
mod tests;
