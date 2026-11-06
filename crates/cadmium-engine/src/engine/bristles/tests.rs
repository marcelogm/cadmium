use super::layout::BristleLayout;
use super::layout::{
    BUNDLE_COVERING_GAIN, BristleRest, FULL_BUNDLE_GAIN_RADIUS_PX, MIN_BRISTLE_RADIUS_PX,
    MIN_BRISTLES, disc_texel_count, layout,
};
use super::roots::{bundle_gain, capped_count_onset_radius_px};
use super::*;
use crate::brush::HeadSpec;
use crate::engine::{TIP_HALF, TIP_SIZE};

fn request(head: HeadSpec, ferrule_radius_px: f32) -> LayoutRequest {
    LayoutRequest {
        head: HeadSpec {
            length: 1.5,
            ..head
        },
        ferrule_radius_px,
        seed: 7,
    }
}

#[test]
fn the_same_seed_lays_out_the_same_bristles() {
    let first = layout(&request(HeadSpec::ROUND, 20.0));
    let second = layout(&request(HeadSpec::ROUND, 20.0));
    assert_eq!(first.bristles, second.bristles);
    assert_eq!(first.owners, second.owners);
    let other = layout(&LayoutRequest {
        seed: 8,
        ..request(HeadSpec::ROUND, 20.0)
    });
    assert_ne!(first.bristles, other.bristles);
}

#[test]
fn a_default_round_brush_has_about_sixty_four_bristles() {
    let count = layout(&request(HeadSpec::ROUND, 20.0)).bristles.len();
    assert!((48..=80).contains(&count), "{count}");
}

#[test]
fn a_huge_brush_stops_at_the_bristle_cap() {
    for head in ALL_HEADS {
        let count = layout(&request(head, 250.0)).bristles.len();
        assert_eq!(count, MAX_BRISTLES);
    }
}

fn default_density() -> f32 {
    HeadSpec::ROUND.density_per_px
}

#[test]
fn bristles_keep_their_width_until_the_count_reaches_its_cap() {
    let density = default_density();
    let onset_px = capped_count_onset_radius_px(density);
    for radius_px in [1.0, 40.0, 100.0, onset_px] {
        assert_eq!(bundle_gain(radius_px, density), 1.0, "at {radius_px} px");
    }
}

#[test]
fn bundles_widen_steadily_past_the_cap_up_to_the_covering_gain() {
    let density = default_density();
    let onset_px = capped_count_onset_radius_px(density);
    let steps = 64;
    let gains: Vec<f32> = (0..=steps)
        .map(|step| onset_px + (FULL_BUNDLE_GAIN_RADIUS_PX - onset_px) * step as f32 / steps as f32)
        .map(|radius_px| bundle_gain(radius_px, density))
        .collect();
    assert!(gains.windows(2).all(|pair| pair[1] >= pair[0]), "{gains:?}");
    assert!(gains[steps / 2] > 1.0 && gains[steps / 2] < BUNDLE_COVERING_GAIN);
    let full = bundle_gain(FULL_BUNDLE_GAIN_RADIUS_PX, density);
    assert!((full - BUNDLE_COVERING_GAIN).abs() < 1e-6, "{full}");
}

#[test]
fn a_tiny_brush_keeps_at_least_three_bristles() {
    let laid = layout(&request(HeadSpec::ROUND, 1.0));
    assert!(laid.bristles.len() >= MIN_BRISTLES);
    assert!(
        laid.bristles
            .iter()
            .all(|bristle| bristle.radius_px >= MIN_BRISTLE_RADIUS_PX)
    );
}

#[test]
fn round_roots_sit_inside_the_ferrule() {
    let laid = layout(&request(HeadSpec::ROUND, 20.0));
    for bristle in &laid.bristles {
        assert!(bristle.root_px[0].hypot(bristle.root_px[1]) <= 20.0);
    }
}

#[test]
fn flat_roots_sit_in_a_thin_bar() {
    let laid = layout(&request(HeadSpec::FLAT, 20.0));
    for bristle in &laid.bristles {
        assert!(bristle.root_px[0].abs() <= 20.0);
        assert!(bristle.root_px[1].abs() <= 20.0 * 0.25 + 1e-3);
    }
}

#[test]
fn fan_bristles_splay_sideways_only() {
    let laid = layout(&request(HeadSpec::FAN, 20.0));
    let limit = 50.0_f32.to_radians().sin() + 1e-3;
    for bristle in &laid.bristles {
        assert!(bristle.splay[0] * bristle.root_px[0] >= 0.0);
        assert!(bristle.splay[0].abs() <= limit);
        assert_eq!(bristle.splay[1], 0.0);
    }
    assert!(laid.bristles.iter().any(|bristle| bristle.splay[0] > 0.3));
    assert!(laid.bristles.iter().any(|bristle| bristle.splay[0] < -0.3));
}

#[test]
fn fan_roots_centre_on_the_handle_axis() {
    let laid = layout(&request(HeadSpec::FAN, 20.0));
    let count = laid.bristles.len() as f32;
    let mean = laid.bristles.iter().fold([0.0f32; 2], |sum, bristle| {
        [sum[0] + bristle.root_px[0], sum[1] + bristle.root_px[1]]
    });
    assert!(mean[0].abs() / count < 1.0, "{mean:?}");
    assert!(mean[1].abs() / count < 1.0, "{mean:?}");
}

#[test]
fn every_texel_of_the_tip_disc_belongs_to_a_bristle() {
    let radius_px = 20.0;
    let laid = layout(&request(HeadSpec::ROUND, radius_px));
    assert_eq!(laid.owners.len(), (TIP_SIZE * TIP_SIZE) as usize);
    let count = laid.bristles.len() as u32;
    let inside = laid.owners.iter().filter(|&&owner| owner < count).count();
    assert_eq!(inside, disc_texel_count(radius_px));
    let outside = laid
        .owners
        .iter()
        .filter(|&&owner| owner == u32::MAX)
        .count();
    assert_eq!(inside + outside, laid.owners.len());
}

const LANE_CHECKS: usize = 8;

fn lane_owners<'a>(
    laid: &'a BristleLayout,
    bristle: &'a BristleRest,
) -> impl Iterator<Item = u32> + 'a {
    (0..LANE_CHECKS).map(move |sample| {
        let along = sample as f32 / (LANE_CHECKS - 1) as f32;
        let x = bristle.lane_start[0] + along * (bristle.lane_end[0] - bristle.lane_start[0]);
        let y = bristle.lane_start[1] + along * (bristle.lane_end[1] - bristle.lane_start[1]);
        laid.owners[y as usize * TIP_SIZE as usize + x as usize]
    })
}

fn lanes_stray(laid: &BristleLayout) -> Vec<usize> {
    laid.bristles
        .iter()
        .enumerate()
        .filter(|(index, bristle)| lane_owners(laid, bristle).any(|owner| owner != *index as u32))
        .map(|(index, _)| index)
        .collect()
}

#[test]
fn every_lane_lies_inside_the_cell_of_its_bristle() {
    for head in ALL_HEADS {
        let laid = layout(&request(head, 20.0));
        assert_eq!(lanes_stray(&laid), Vec::<usize>::new(), "{head:?}");
    }
}

#[test]
fn the_largest_brush_lays_out_every_bristle() {
    let laid = layout(&request(HeadSpec::ROUND, 250.0));
    assert_eq!(laid.bristles.len(), MAX_BRISTLES);
}

const EDGE_RADII_PX: [f32; 3] = [1.0, 20.0, 250.0];
const ALL_HEADS: [HeadSpec; 3] = [HeadSpec::ROUND, HeadSpec::FLAT, HeadSpec::FAN];

fn tip_distance_px2(bristle: &BristleRest, x: usize, y: usize) -> f32 {
    let half = TIP_HALF as f32;
    let dx = half + bristle.root_px[0] - (x as f32 + 0.5);
    let dy = half + bristle.root_px[1] - (y as f32 + 0.5);
    dx * dx + dy * dy
}

fn texel_is_in_disc(x: usize, y: usize, radius_px: f32) -> bool {
    let half = TIP_HALF as i32;
    let (dx, dy) = ((x as i32 - half) as f32, (y as i32 - half) as f32);
    dx * dx + dy * dy <= radius_px * radius_px
}

fn nearest_by_brute_force(bristles: &[BristleRest], x: usize, y: usize) -> (usize, f32) {
    bristles
        .iter()
        .enumerate()
        .map(|(index, bristle)| (index, tip_distance_px2(bristle, x, y)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("a layout has bristles")
}

struct OwnerCensus {
    natural: Vec<usize>,
    mismatches: Vec<usize>,
}

fn census_texel(laid: &BristleLayout, census: &mut OwnerCensus, x: usize, y: usize) {
    let owner = laid.owners[y * TIP_SIZE as usize + x];
    let (nearest, distance) = nearest_by_brute_force(&laid.bristles, x, y);
    census.natural[nearest] += 1;
    if tip_distance_px2(&laid.bristles[owner as usize], x, y) != distance {
        census.mismatches.push(owner as usize);
    }
}

fn check_owner_map(head: HeadSpec, radius_px: f32) {
    let laid = layout(&request(head, radius_px));
    let size = TIP_SIZE as usize;
    let mut census = OwnerCensus {
        natural: vec![0; laid.bristles.len()],
        mismatches: Vec::new(),
    };
    let texels = (0..size).flat_map(|y| (0..size).map(move |x| (x, y)));
    for (x, y) in texels {
        if texel_is_in_disc(x, y, radius_px) {
            census_texel(&laid, &mut census, x, y);
        } else {
            assert_eq!(
                laid.owners[y * size + x],
                u32::MAX,
                "{head:?} {radius_px} ({x},{y})"
            );
        }
    }
    for thief in census.mismatches {
        assert_eq!(census.natural[thief], 0, "{head:?} {radius_px}");
    }
}

#[test]
fn the_owner_map_is_the_nearest_root_except_texels_given_to_rootless_bristles() {
    for head in ALL_HEADS {
        for radius_px in EDGE_RADII_PX {
            check_owner_map(head, radius_px);
        }
    }
}

#[test]
fn every_bristle_owns_a_texel_and_every_lane_stays_in_its_cell_at_every_size() {
    for head in ALL_HEADS {
        for radius_px in EDGE_RADII_PX {
            let laid = layout(&request(head, radius_px));
            assert!(laid.bristles.len() >= MIN_BRISTLES.min(disc_texel_count(radius_px)));
            let ownerless =
                (0..laid.bristles.len()).find(|&index| !laid.owners.contains(&(index as u32)));
            assert_eq!(ownerless, None, "{head:?} {radius_px}");
            assert_eq!(
                lanes_stray(&laid),
                Vec::<usize>::new(),
                "{head:?} {radius_px}"
            );
        }
    }
}

#[test]
fn a_disc_with_fewer_texels_than_bristles_gets_one_bristle_per_texel() {
    let laid = layout(&request(HeadSpec::ROUND, 0.5));
    assert_eq!(laid.bristles.len(), disc_texel_count(0.5));
    for index in 0..laid.bristles.len() {
        assert!(laid.owners.contains(&(index as u32)));
    }
}

fn texels_inside_by_scanning(radius_px: f32) -> usize {
    let half = TIP_HALF as i32;
    let size = crate::engine::TIP_SIZE as i32;
    let radius_squared = radius_px * radius_px;
    (-half..size - half)
        .flat_map(|y| (-half..size - half).map(move |x| (x as f32, y as f32)))
        .filter(|(x, y)| x * x + y * y <= radius_squared)
        .count()
}

#[test]
fn the_disc_texel_count_matches_a_scan_of_every_tip_texel() {
    let radii = (0..=2000).map(|step| 0.5 + step as f32 * 0.137);
    for radius_px in radii.chain([0.0, 0.25, 1.0, 1.5, 2.0, 255.5, 256.0, 300.0, 400.0]) {
        assert_eq!(
            disc_texel_count(radius_px),
            texels_inside_by_scanning(radius_px),
            "radius {radius_px}"
        );
    }
}

#[test]
fn the_reach_grows_with_the_hair_length() {
    let short = HeadSpec {
        length: 1.0,
        ..HeadSpec::ROUND
    };
    let long = HeadSpec {
        length: 3.0,
        ..HeadSpec::ROUND
    };
    assert!(bristle_reach_px(20.0, &long) > bristle_reach_px(20.0, &short));
}

#[test]
fn a_fan_reaches_at_least_as_far_as_its_flat_counterpart() {
    let flat = HeadSpec {
        fan_deg: 0.0,
        ..HeadSpec::FAN
    };
    assert!(bristle_reach_px(20.0, &HeadSpec::FAN) >= bristle_reach_px(20.0, &flat));
}

#[test]
fn the_ferrule_radius_is_half_the_diameter_within_the_tip() {
    use crate::engine::{MAX_TIP_RADIUS, MIN_DAB_RADIUS, ferrule_radius_for};
    assert_eq!(ferrule_radius_for(40.0), 20.0);
    assert_eq!(ferrule_radius_for(0.0), MIN_DAB_RADIUS);
    assert_eq!(ferrule_radius_for(10.0 * TIP_SIZE as f32), MAX_TIP_RADIUS);
}
