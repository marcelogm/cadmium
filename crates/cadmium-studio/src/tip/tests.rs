use cadmium_engine::brush::{HairSpec, HeadSpec};

use super::{DRY_POINT_SHARE, FERRULE_HEIGHT, FERRULE_MOUTH, POINT_PULL, TipGeometry, TipView};

const CLOSE: f32 = 1e-5;

fn tip(head: HeadSpec, hair: HairSpec) -> TipGeometry {
    TipGeometry::new(&head, &hair)
}

/// A head with every control at one end of its range, the ends chosen by the bits of `corner`.
fn extreme_head(corner: u32) -> HeadSpec {
    let pick = |range: [f32; 2], bit: u32| range[((corner >> bit) & 1) as usize];
    HeadSpec {
        thickness: pick(HeadSpec::THICKNESS_RANGE, 0),
        length: pick(HeadSpec::LENGTH_RANGE, 1),
        dome: pick(HeadSpec::DOME_RANGE, 2),
        slant: pick(HeadSpec::SLANT_RANGE, 3),
        point: pick(HeadSpec::POINT_RANGE, 4),
        belly: pick(HeadSpec::BELLY_RANGE, 5),
        fan_deg: pick(HeadSpec::FAN_RANGE_DEG, 6),
        density_per_px: pick(HeadSpec::DENSITY_RANGE_PER_PX, 7),
    }
}

fn every_point(tip: &TipGeometry) -> impl Iterator<Item = [f32; 2]> + '_ {
    [&tip.front, &tip.side]
        .into_iter()
        .flat_map(|view| {
            view.outline
                .iter()
                .chain(view.hairs.iter().flatten())
                .chain(&view.ferrule.outline)
                .chain(&view.ferrule.handle)
                .copied()
        })
        .chain(tip.footprint.iter().copied())
}

fn round_tip_width(wet_point: f32) -> f32 {
    let hair = HairSpec {
        wet_point,
        ..HairSpec::SABLE
    };
    tip(HeadSpec::ROUND, hair).dimensions.tip_width
}

#[test]
fn a_round_head_converges_to_a_point() {
    let round = tip(HeadSpec::ROUND, HairSpec::SABLE);
    assert!(
        round.dimensions.tip_width < 0.2 * 2.0,
        "{:?}",
        round.dimensions
    );
    let edge = round.side.tip_edge();
    let thickness_at_tip = edge[edge.len() - 1][0] - edge[0][0];
    assert!(thickness_at_tip < 0.2 * 2.0, "{thickness_at_tip}");
}

#[test]
fn a_flat_head_keeps_its_width_and_a_straight_tip() {
    let flat = tip(HeadSpec::FLAT, HairSpec::HOG);
    let edge = flat.front.tip_edge();
    assert!(
        edge.iter()
            .all(|point| (point[1] - HeadSpec::FLAT.length).abs() < CLOSE),
        "{edge:?}"
    );
    assert!((flat.dimensions.tip_width - 2.0).abs() < CLOSE);
}

#[test]
fn the_wet_point_sets_how_far_a_round_head_converges() {
    assert!((round_tip_width(1.0) - 2.0 * (1.0 - POINT_PULL)).abs() < CLOSE);
    let dry = 2.0 * (1.0 - POINT_PULL * DRY_POINT_SHARE);
    assert!((round_tip_width(0.0) - dry).abs() < CLOSE);
    let half = round_tip_width(0.5);
    assert!(
        half < round_tip_width(0.0) && half > round_tip_width(1.0),
        "{half}"
    );
}

#[test]
fn a_fan_widens_with_its_angle_and_stays_finite_at_ninety_degrees() {
    let widths: Vec<f32> = [0.0, 15.0, 30.0, 45.0, 60.0, 75.0, 90.0]
        .into_iter()
        .map(|fan_deg| {
            let head = HeadSpec {
                fan_deg,
                ..HeadSpec::FAN
            };
            tip(head, HairSpec::HOG).dimensions.tip_width
        })
        .collect();
    assert!(
        widths.windows(2).all(|pair| pair[1] > pair[0]),
        "{widths:?}"
    );
    let wide = tip(
        HeadSpec {
            fan_deg: 90.0,
            ..HeadSpec::FAN
        },
        HairSpec::HOG,
    );
    let reach = 1.0 + HeadSpec::FAN.length;
    assert!(wide.front.outline.iter().all(|p| p[0].abs() <= reach));
    let edge = wide.front.tip_edge();
    assert!((edge[edge.len() - 1][0] - wide.dimensions.tip_width / 2.0).abs() < CLOSE);
}

#[test]
fn at_ninety_degrees_the_outermost_hair_turns_by_forty_five() {
    let wide = tip(
        HeadSpec {
            fan_deg: 90.0,
            ..HeadSpec::FAN
        },
        HairSpec::HOG,
    );
    let root = wide.front.outline[wide.front.outline.len() - 1];
    let edge = wide.front.tip_edge();
    let end = edge[edge.len() - 1];
    let turn = (end[0] - root[0]).atan2(end[1] - root[1]);
    assert!((turn - std::f32::consts::FRAC_PI_4).abs() < 1e-3, "{turn}");
}

#[test]
fn a_slant_tilts_the_tip_line_towards_its_sign() {
    let angular = tip(HeadSpec::ANGULAR, HairSpec::SYNTHETIC);
    let edge = angular.front.tip_edge();
    assert!(
        edge.windows(2).all(|pair| pair[1][1] > pair[0][1]),
        "{edge:?}"
    );
    assert!(edge[edge.len() - 1][1] > edge[0][1] + 0.5);
    let mirrored = tip(
        HeadSpec {
            slant: -HeadSpec::ANGULAR.slant,
            ..HeadSpec::ANGULAR
        },
        HairSpec::SYNTHETIC,
    );
    let edge = mirrored.front.tip_edge();
    assert!(edge[0][1] > edge[edge.len() - 1][1] + 0.5);
}

#[test]
fn a_filbert_dome_rounds_the_tip() {
    let filbert = tip(HeadSpec::FILBERT, HairSpec::HOG);
    let edge = filbert.front.tip_edge();
    let middle = edge.len() / 2;
    assert!(
        edge[..=middle]
            .windows(2)
            .all(|pair| pair[1][1] >= pair[0][1])
    );
    assert!(
        edge[middle..]
            .windows(2)
            .all(|pair| pair[1][1] <= pair[0][1])
    );
    assert!(edge[middle][1] > edge[0][1] + 0.5, "{edge:?}");
}

#[test]
fn a_flat_footprint_is_wider_than_thick() {
    let flat = tip(HeadSpec::FLAT, HairSpec::HOG);
    let span = |axis: usize| {
        let values = flat.footprint.iter().map(|point| point[axis]);
        values.clone().fold(f32::MIN, f32::max) - values.fold(f32::MAX, f32::min)
    };
    assert!(span(0) > 3.0 * span(1), "{} {}", span(0), span(1));
}

#[test]
fn the_roots_sit_on_the_ferrule_mouth() {
    let flat = tip(HeadSpec::FLAT, HairSpec::HOG);
    let front = &flat.front.outline;
    assert_eq!(front[0], [-1.0, 0.0]);
    assert_eq!(front[front.len() - 1], [1.0, 0.0]);
    assert_eq!(flat.side.outline[0], [-HeadSpec::FLAT.thickness, 0.0]);
    let ferrule = &flat.front.ferrule;
    assert!((ferrule.mouth - FERRULE_MOUTH).abs() < CLOSE);
    let side_mouth = FERRULE_MOUTH * HeadSpec::FLAT.thickness;
    assert!((flat.side.ferrule.mouth - side_mouth).abs() < CLOSE);
    assert_eq!(ferrule.outline[0], [-ferrule.mouth, 0.0]);
    assert!(ferrule.outline.iter().all(|point| point[1] <= 0.0));
    assert!(ferrule.handle[1][1] < ferrule.handle[0][1]);
    assert!(
        ferrule
            .crimp
            .iter()
            .all(|&y| y < 0.0 && y > ferrule.handle[0][1])
    );
}

/// The x of the polyline `edge` (running down, y rising) at `y`, by linear interpolation.
fn edge_x_at(edge: &[[f32; 2]], y: f32) -> f32 {
    let pair = edge
        .windows(2)
        .find(|pair| pair[0][1] <= y && y <= pair[1][1])
        .expect("y within the edge");
    let share = (y - pair[0][1]) / (pair[1][1] - pair[0][1]).max(f32::EPSILON);
    pair[0][0] + share * (pair[1][0] - pair[0][0])
}

/// Every guide point lies strictly between the left and right edges at its height, and the
/// guides run left to right.
fn assert_guides_between_edges(view: &TipView) {
    let corners = view.outline.len() - view.tip_edge().len();
    let left = &view.outline[..=corners / 2];
    let right: Vec<[f32; 2]> = view.outline[corners / 2 + view.tip_edge().len() - 1..]
        .iter()
        .rev()
        .copied()
        .collect();
    for hair in &view.hairs {
        for &[x, y] in hair {
            assert!(
                edge_x_at(left, y) < x && x < edge_x_at(&right, y),
                "{x} {y}"
            );
        }
    }
    for pair in view.hairs.windows(2) {
        assert!(pair[0].iter().zip(&pair[1]).all(|(a, b)| a[0] < b[0]));
    }
}

#[test]
fn guide_lines_run_between_the_outline_edges() {
    let flat = tip(HeadSpec::FLAT, HairSpec::HOG);
    assert_eq!(flat.front.hairs.len(), 12);
    assert!(flat.front.hairs.iter().all(|hair| hair.len() == 15));
    assert_eq!(flat.side.hairs.len(), 2);
    let round = tip(HeadSpec::ROUND, HairSpec::SYNTHETIC);
    assert_eq!(round.side.hairs.len(), 12);
    for view in [&flat.front, &flat.side, &round.front, &round.side] {
        assert_guides_between_edges(view);
    }
}

#[test]
fn the_tip_width_is_the_drawn_tip_edge_span() {
    let head = HeadSpec {
        fan_deg: 60.0,
        dome: 1.0,
        slant: 0.5,
        ..HeadSpec::FAN
    };
    let drawn = tip(head, HairSpec::HOG);
    let xs = drawn.front.tip_edge().iter().map(|point| point[0]);
    let span = xs.clone().fold(f32::MIN, f32::max) - xs.fold(f32::MAX, f32::min);
    assert!((drawn.dimensions.tip_width - span).abs() < CLOSE, "{span}");
    let nominal = 2.0 * (1.0 + (head.fan_deg / 2.0).to_radians().sin() * head.length);
    assert!(
        drawn.dimensions.tip_width < nominal,
        "{} {nominal}",
        drawn.dimensions.tip_width
    );
}

#[test]
fn the_extent_holds_the_handle_of_a_thin_side_view() {
    let side = tip(HeadSpec::FLAT, HairSpec::HOG).side;
    let [min, max] = side.extent();
    let [base, cut] = side.ferrule.handle;
    for [x, y] in [base, cut] {
        assert!(min[0] <= -x && x <= max[0] && min[1] <= y && y <= max[1]);
    }
    assert!(side.bounds()[0][1] > cut[1], "bounds leave the handle out");
    assert!(side.bounds()[1][0] <= max[0]);
}

#[test]
fn the_footprint_bounds_hold_every_footprint_point() {
    let flat = tip(HeadSpec::FLAT, HairSpec::HOG);
    let [min, max] = flat.footprint_bounds();
    assert!(
        flat.footprint
            .iter()
            .all(|&[x, y]| { min[0] <= x && x <= max[0] && min[1] <= y && y <= max[1] })
    );
    assert!((max[0] - min[0] - flat.dimensions.tip_width).abs() < 1e-3);
}

#[test]
fn a_view_built_by_hand_has_no_tip_edge_instead_of_panicking() {
    let mut view = tip(HeadSpec::ROUND, HairSpec::SABLE).front;
    view.outline.truncate(3);
    assert!(view.tip_edge().is_empty());
}

#[test]
fn the_bounds_hold_the_ferrule_and_the_hairs() {
    let round = tip(HeadSpec::ROUND, HairSpec::SYNTHETIC);
    let [min, max] = round.front.bounds();
    assert!((min[0] + FERRULE_MOUTH).abs() < CLOSE && (max[0] - FERRULE_MOUTH).abs() < CLOSE);
    assert!((min[1] + FERRULE_HEIGHT).abs() < CLOSE);
    assert!((max[1] - HeadSpec::ROUND.length).abs() < CLOSE);
}

/// Finite everywhere, hairs below the ferrule with a positive length, a ferrule with width.
fn assert_drawable(head: HeadSpec, hair: HairSpec) {
    let drawn = tip(head, hair);
    assert!(
        every_point(&drawn).all(|[x, y]| x.is_finite() && y.is_finite()),
        "{head:?}"
    );
    let mut hairs = drawn.front.outline.iter().chain(&drawn.side.outline);
    assert!(hairs.all(|point| point[1] >= 0.0), "{head:?}");
    assert!(
        drawn.front.tip_edge().iter().all(|p| p[1] > 0.0),
        "{head:?}"
    );
    assert!(
        drawn.side.ferrule.outline.iter().all(|p| p[0] != 0.0),
        "{head:?}"
    );
    let d = drawn.dimensions;
    let values = [d.length, d.tip_width, d.thickness, d.fan_deg, d.slant];
    assert!(values.iter().all(|value| value.is_finite()), "{head:?}");
}

#[test]
fn every_corner_of_every_range_draws_finite_hairs_below_the_ferrule() {
    for corner in 0..256 {
        for wet_point in HairSpec::WET_POINT_RANGE {
            let hair = HairSpec {
                wet_point,
                ..HairSpec::SYNTHETIC
            };
            assert_drawable(extreme_head(corner), hair);
        }
    }
}

#[test]
fn nonsense_values_are_clamped_before_drawing() {
    let wild = HeadSpec {
        thickness: f32::NAN,
        fan_deg: f32::INFINITY,
        length: -3.0,
        ..HeadSpec::ROUND
    };
    let hair = HairSpec {
        wet_point: f32::NAN,
        ..HairSpec::SYNTHETIC
    };
    let drawn = tip(wild, hair);
    assert!(every_point(&drawn).all(|[x, y]| x.is_finite() && y.is_finite()));
    assert_eq!(drawn.dimensions.fan_deg, HeadSpec::FAN_RANGE_DEG[1]);
    assert_eq!(drawn.dimensions.length, HeadSpec::LENGTH_RANGE[0]);
}
