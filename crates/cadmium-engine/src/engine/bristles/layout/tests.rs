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
