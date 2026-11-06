use super::{Root, SplitMix, bristle_count, covered_area_px2, place_roots};
use crate::brush::HeadSpec;

const RADIUS_PX: f32 = 40.0;
const SEED: u64 = 7;
/// Points along a control's range where a nudge is tried.
const SWEEP_STEPS: usize = 400;
/// A nudge, as a share of the control's range.
const NUDGE_SHARE: f32 = 1e-4;
/// The most a nudge may move a root (in ferrule radii), change its splay, or turn its lane
/// (the distance between the unit lane directions, so a flipped lane counts as 2).
const MAX_ROOT_SHIFT: f32 = 0.01;
/// The most a nudge may change a hair's length share: the dome's rim is steep, not broken.
const MAX_SHARE_SHIFT: f32 = 0.1;
/// The most a nudge may change the covered area, as a share of it.
const MAX_AREA_SHIFT: f32 = 0.01;

fn roots_of(head: &HeadSpec) -> Vec<Root> {
    let count = bristle_count(RADIUS_PX, head.density_per_px);
    place_roots(head, RADIUS_PX, count, &mut SplitMix::new(SEED))
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn finite(root: &Root) -> bool {
    root.position_px
        .iter()
        .chain(&root.splay)
        .chain(&root.lane_direction)
        .chain([&root.length_share])
        .all(|value| value.is_finite())
}

/// The largest root move (or splay or lane change) and the largest length-share change
/// between two layouts of the same count. The lane is compared with its sign: a lane is a ray
/// from the root through its cell, so a flipped one is a different lane.
fn largest_shift(a: &[Root], b: &[Root]) -> (f32, f32) {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).fold((0.0, 0.0), |(moved, share), (p, q)| {
        assert!(finite(p) && finite(q));
        let position = distance(p.position_px, q.position_px) / RADIUS_PX;
        let splay = distance(p.splay, q.splay);
        let lane = distance(p.lane_direction, q.lane_direction);
        let length = (p.length_share - q.length_share).abs();
        (moved.max(position).max(splay).max(lane), share.max(length))
    })
}

/// Nudges one control of `base` across its whole `range`, the last nudge ending exactly on
/// the range's end, and asserts each nudge changes the roots and the area only a little.
fn assert_continuous(base: HeadSpec, range: [f32; 2], set: fn(&mut HeadSpec, f32)) {
    let span = range[1] - range[0];
    let nudge = span * NUDGE_SHARE;
    for step in 0..=SWEEP_STEPS {
        let value = range[0] + (span - nudge) * step as f32 / SWEEP_STEPS as f32;
        let nudged = if step == SWEEP_STEPS {
            range[1]
        } else {
            value + nudge
        };
        let [mut before, mut after] = [base; 2];
        set(&mut before, value);
        set(&mut after, nudged);
        let (moved, share) = largest_shift(&roots_of(&before), &roots_of(&after));
        let area = [before, after].map(|head| covered_area_px2(&head, RADIUS_PX));
        assert!(
            moved <= MAX_ROOT_SHIFT,
            "{value} → {nudged}: roots moved {moved}, {before:?}"
        );
        assert!(
            share <= MAX_SHARE_SHIFT,
            "{value} → {nudged}: share moved {share}, {before:?}"
        );
        let grown = (area[1] - area[0]).abs() / area[0];
        assert!(
            grown <= MAX_AREA_SHIFT,
            "{value} → {nudged}: area {area:?}, {before:?}"
        );
    }
}

#[test]
fn thickness_moves_the_roots_a_little_at_a_time() {
    let thicken = |head: &mut HeadSpec, value: f32| head.thickness = value;
    let fanned_flat = HeadSpec {
        fan_deg: 30.0,
        ..HeadSpec::FLAT
    };
    let shaped_fan = HeadSpec {
        dome: 0.8,
        slant: 0.5,
        ..HeadSpec::FAN
    };
    for base in [
        HeadSpec::FLAT,
        HeadSpec::FILBERT,
        fanned_flat,
        shaped_fan,
        HeadSpec::MOP,
    ] {
        assert_continuous(base, HeadSpec::THICKNESS_RANGE, thicken);
    }
}

#[test]
fn the_fan_moves_the_roots_a_little_at_a_time() {
    let open = |head: &mut HeadSpec, value: f32| head.fan_deg = value;
    let thick_flat = HeadSpec {
        thickness: 0.9,
        ..HeadSpec::FLAT
    };
    for base in [
        HeadSpec::FLAT,
        HeadSpec::FILBERT,
        thick_flat,
        HeadSpec::DAGGER,
        HeadSpec::ROUND,
    ] {
        assert_continuous(base, HeadSpec::FAN_RANGE_DEG, open);
    }
}

#[test]
fn every_section_draws_the_same_randomness_per_root() {
    use crate::brush::Section;
    let head = HeadSpec {
        thickness: 0.9,
        fan_deg: 5.0,
        ..HeadSpec::FLAT
    };
    let slot = super::RootSlot {
        index: 3,
        count: 64,
        radius_px: RADIUS_PX,
    };
    let ends = [Section::Round, Section::Bar, Section::Arc].map(|section| {
        let mut random = SplitMix::new(SEED);
        super::section_root(section, &head, slot, &mut random);
        random
    });
    assert!(ends.windows(2).all(|pair| pair[0] == pair[1]), "{ends:?}");
}

#[test]
fn a_blended_head_lays_finite_unit_lanes() {
    let head = HeadSpec {
        thickness: 0.9,
        fan_deg: 5.0,
        ..HeadSpec::FILBERT
    };
    for root in roots_of(&head) {
        assert!(finite(&root));
        let length = root.lane_direction[0].hypot(root.lane_direction[1]);
        assert!((length - 1.0).abs() < 1e-4, "{:?}", root.lane_direction);
    }
}
