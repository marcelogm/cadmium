use super::{BristleLayout, LayoutRequest, layout};
use crate::brush::HeadSpec;

const REFERENCE_SEED: u64 = 0x00C4_D317_B215_7E55;
const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
const REFERENCE_RADII_PX: [f32; 2] = [20.0, 250.0];
const REFERENCE_FINGERPRINTS: [u64; 6] = [
    0x594a_bad9_9ce2_5b25,
    0x5b59_a02d_093c_7ac0,
    0x8e89_0549_0040_9312,
    0xbc67_3f4d_3400_b197,
    0xb6e0_90a2_960b_a9df,
    0x2740_9292_b9bc_3b26,
];

fn fnv(hash: u64, word: u32) -> u64 {
    (hash ^ u64::from(word)).wrapping_mul(FNV_PRIME)
}

fn fingerprint(laid: &BristleLayout) -> u64 {
    let floats = laid.bristles.iter().flat_map(|b| {
        [
            b.root_px[0],
            b.root_px[1],
            b.splay[0],
            b.splay[1],
            b.length_px,
            b.stiffness,
            b.radius_px,
            b.lane_start[0],
            b.lane_start[1],
            b.lane_end[0],
            b.lane_end[1],
        ]
        .map(f32::to_bits)
    });
    let words = laid
        .bristles
        .iter()
        .map(|b| b.cluster)
        .chain(laid.owners.iter().copied())
        .chain(laid.ranks.iter().copied())
        .chain(laid.cell_grids.iter().copied())
        .chain(laid.cluster_index.iter().copied());
    floats.chain(words).fold(FNV_OFFSET, fnv)
}

fn reference_layout(head: HeadSpec, radius_px: f32) -> BristleLayout {
    layout(&LayoutRequest {
        head,
        ferrule_radius_px: radius_px,
        seed: REFERENCE_SEED,
    })
}

fn reference_fingerprints() -> Vec<u64> {
    REFERENCE_RADII_PX
        .iter()
        .flat_map(|&radius_px| {
            [HeadSpec::ROUND, HeadSpec::FLAT, HeadSpec::FAN]
                .map(|head| fingerprint(&reference_layout(head, radius_px)))
        })
        .collect()
}

#[test]
fn todays_round_flat_and_fan_layouts_stay_bit_identical() {
    let prints = reference_fingerprints();
    assert_eq!(prints, REFERENCE_FINGERPRINTS, "{prints:#x?}");
}

const PROFILE_RADIUS_PX: f32 = 40.0;

fn head_layout(head: HeadSpec) -> BristleLayout {
    layout(&LayoutRequest {
        head,
        ferrule_radius_px: PROFILE_RADIUS_PX,
        seed: 7,
    })
}

fn mean_length_px(laid: &BristleLayout, keep: impl Fn(f32) -> bool) -> f32 {
    let kept: Vec<f32> = laid
        .bristles
        .iter()
        .filter(|b| keep(b.root_px[0] / PROFILE_RADIUS_PX))
        .map(|b| b.length_px)
        .collect();
    assert!(!kept.is_empty());
    kept.iter().sum::<f32>() / kept.len() as f32
}

#[test]
fn a_domed_head_has_shorter_hairs_at_its_edges() {
    let laid = head_layout(HeadSpec {
        dome: 1.0,
        ..HeadSpec::FLAT
    });
    let centre = mean_length_px(&laid, |x| x.abs() < 0.25);
    let edge = mean_length_px(&laid, |x| x.abs() > 0.75);
    assert!(edge < 0.8 * centre, "edge {edge} centre {centre}");
}

#[test]
fn a_slanted_head_grows_its_hairs_across_its_width() {
    let laid = head_layout(HeadSpec {
        slant: 1.0,
        ..HeadSpec::FLAT
    });
    let quarters: Vec<f32> = [-1.0f32, -0.5, 0.0, 0.5]
        .iter()
        .map(|&from| mean_length_px(&laid, |x| x >= from && x < from + 0.5))
        .collect();
    assert!(
        quarters.windows(2).all(|pair| pair[1] > pair[0]),
        "{quarters:?}"
    );
}

#[test]
fn reach_still_bounds_every_hair_of_a_shaped_head() {
    for head in [
        HeadSpec::FILBERT,
        HeadSpec::ANGULAR,
        HeadSpec::DAGGER,
        HeadSpec::RIGGER,
    ] {
        let laid = head_layout(head);
        let bound = super::reach_bound_px(&super::ReachRequest {
            ferrule_radius_px: PROFILE_RADIUS_PX,
            head,
        });
        let longest = laid
            .bristles
            .iter()
            .map(|b| b.length_px)
            .fold(0.0, f32::max);
        assert!(longest + PROFILE_RADIUS_PX <= bound, "{head:?}");
    }
}

fn outward(b: &super::BristleRest) -> f32 {
    b.splay[0] * b.root_px[0] + b.splay[1] * b.root_px[1]
}

#[test]
fn a_bellied_round_head_splays_its_hairs_outward() {
    let laid = head_layout(HeadSpec {
        belly: 1.0,
        ..HeadSpec::ROUND
    });
    assert!(laid.bristles.iter().all(|b| outward(b) >= 0.0));
    let rim: Vec<_> = laid
        .bristles
        .iter()
        .filter(|b| b.root_px[0].hypot(b.root_px[1]) > 0.5 * PROFILE_RADIUS_PX)
        .collect();
    assert!(rim.iter().all(|b| outward(b) > 0.0));
}

#[test]
fn a_bellied_fan_still_splays_sideways_only() {
    let laid = head_layout(HeadSpec {
        belly: 1.0,
        ..HeadSpec::FAN
    });
    assert!(laid.bristles.iter().all(|b| b.splay[1] == 0.0));
}

#[test]
fn a_head_without_belly_or_round_fan_keeps_todays_splay() {
    for head in [HeadSpec::ROUND, HeadSpec::FLAT] {
        assert!(
            head_layout(head)
                .bristles
                .iter()
                .all(|b| b.splay == [0.0, 0.0])
        );
    }
}
