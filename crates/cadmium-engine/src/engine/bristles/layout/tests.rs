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

/// The other six factory heads, fingerprinted before the head sections were blended.
const SHAPED_HEADS: [HeadSpec; 6] = [
    HeadSpec::BRIGHT,
    HeadSpec::FILBERT,
    HeadSpec::RIGGER,
    HeadSpec::ANGULAR,
    HeadSpec::DAGGER,
    HeadSpec::MOP,
];
const SHAPED_FINGERPRINTS: [u64; 12] = [
    0x6279_b06d_79d0_d573,
    0x48e5_e12d_12f8_bac8,
    0x3cef_2cd3_0407_3fa3,
    0x998a_943a_9766_76a5,
    0x860e_2199_54e4_6fd4,
    0x75d0_98ac_0a45_f6fd,
    0xadf6_b075_cd0d_1fa1,
    0x17f6_f20e_5d08_971a,
    0xe889_9493_34db_2dc0,
    0x74b6_9374_5f1f_40bf,
    0xab7b_54ef_9f7f_1ffd,
    0xa32f_bcbe_8db6_09a2,
];

fn shaped_fingerprints() -> Vec<u64> {
    REFERENCE_RADII_PX
        .iter()
        .flat_map(|&radius_px| {
            SHAPED_HEADS.map(|head| fingerprint(&reference_layout(head, radius_px)))
        })
        .collect()
}

#[test]
fn the_six_shaped_factory_layouts_stay_bit_identical() {
    let prints = shaped_fingerprints();
    assert_eq!(prints, SHAPED_FINGERPRINTS, "{prints:#x?}");
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

fn mean_ratio(pairs: &[(f32, f32)], keep: impl Fn(f32) -> bool) -> f32 {
    let kept: Vec<f32> = pairs
        .iter()
        .filter(|&&(x, _)| keep(x))
        .map(|&(_, ratio)| ratio)
        .collect();
    assert!(!kept.is_empty());
    kept.iter().sum::<f32>() / kept.len() as f32
}

#[test]
fn a_slanted_head_grows_its_hairs_across_its_width() {
    let plain = head_layout(HeadSpec::FLAT);
    let slanted = head_layout(HeadSpec {
        slant: 1.0,
        ..HeadSpec::FLAT
    });
    assert_eq!(plain.bristles.len(), slanted.bristles.len());
    let pairs: Vec<(f32, f32)> = plain
        .bristles
        .iter()
        .zip(&slanted.bristles)
        .map(|(p, s)| {
            assert_eq!(p.root_px, s.root_px);
            (p.root_px[0] / PROFILE_RADIUS_PX, s.length_px / p.length_px)
        })
        .collect();
    let left = mean_ratio(&pairs, |x| x < -0.5);
    let right = mean_ratio(&pairs, |x| x >= 0.5);
    assert!(left < 0.5 * right, "left {left} right {right}");
    assert!(right > 0.75, "right {right}");
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
    assert!(!rim.is_empty());
    assert!(rim.iter().all(|b| outward(b) > 0.0));
}

#[test]
fn a_bellied_fan_still_splays_sideways_only() {
    let laid = head_layout(HeadSpec {
        belly: 1.0,
        ..HeadSpec::FAN
    });
    assert!(laid.bristles.iter().all(|b| b.splay[1] == 0.0));
    let plain = head_layout(HeadSpec::FAN);
    assert_eq!(plain.bristles.len(), laid.bristles.len());
    let mut rim = 0;
    for (p, b) in plain.bristles.iter().zip(&laid.bristles) {
        assert_eq!(p.root_px, b.root_px);
        assert!(outward(b) >= outward(p), "{b:?} vs {p:?}");
        if b.root_px[0].abs() > 0.5 * PROFILE_RADIUS_PX {
            rim += 1;
            assert!(outward(b) > outward(p), "{b:?} vs {p:?}");
        }
    }
    assert!(rim > 0);
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
