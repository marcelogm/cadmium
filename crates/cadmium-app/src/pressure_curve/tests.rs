use super::*;

fn curve(points: &[(f32, f32)]) -> PressureCurve {
    PressureCurve::from_points(points).expect("valid curve")
}

fn grid() -> impl Iterator<Item = f32> {
    (0..=1000).map(|step| step as f32 / 1000.0)
}

#[test]
fn the_linear_curve_hands_pressure_through_unchanged() {
    let linear = PressureCurve::linear();
    for pressure in grid() {
        assert_eq!(linear.apply(pressure), pressure);
    }
    assert_eq!(PressureCurve::default(), linear);
}

#[test]
fn the_end_points_set_the_floor_and_the_ceiling() {
    let raised = curve(&[(0.0, 0.2), (1.0, 0.8)]);
    assert_eq!(raised.apply(0.0), 0.2);
    assert_eq!(raised.apply(1.0), 0.8);
    assert!((raised.apply(0.5) - 0.5).abs() < 1e-6);
}

#[test]
fn pressure_outside_the_unit_range_is_clamped_first() {
    let raised = curve(&[(0.0, 0.2), (1.0, 0.8)]);
    assert_eq!(raised.apply(-1.0), 0.2);
    assert_eq!(raised.apply(3.0), 0.8);
}

#[test]
fn the_curve_passes_through_its_points() {
    let soft = curve(&[(0.0, 0.0), (0.3, 0.6), (0.7, 0.85), (1.0, 1.0)]);
    for &(x, y) in soft.points() {
        assert!((soft.apply(x) - y).abs() < 1e-6, "at {x}");
    }
}

/// A steep step next to a flat run is where a plain cubic spline overshoots.
#[test]
fn rising_points_give_a_curve_that_never_dips_or_leaves_the_unit_range() {
    let packed: Vec<(f32, f32)> = (0..MAX_POINTS - 1)
        .map(|i| (i as f32 * MIN_GAP, if i < 3 { 0.0 } else { 0.9 }))
        .chain([(1.0, 1.0)])
        .collect();
    let curves = [
        curve(&[(0.0, 0.0), (0.1, 0.9), (0.15, 0.92), (1.0, 1.0)]),
        curve(&[(0.0, 0.0), (0.8, 0.05), (0.85, 0.95), (1.0, 1.0)]),
        curve(&packed),
    ];
    for steep in curves {
        let mut previous = 0.0;
        for pressure in grid() {
            let out = steep.apply(pressure);
            assert!((0.0..=1.0).contains(&out), "{out} at {pressure}");
            assert!(out >= previous - 1e-6, "dips at {pressure}");
            previous = out;
        }
    }
}

#[test]
fn falling_points_give_a_curve_that_never_rises_or_leaves_the_unit_range() {
    let falling = curve(&[(0.0, 1.0), (0.1, 0.1), (0.15, 0.08), (1.0, 0.0)]);
    let mut previous = 1.0;
    for pressure in grid() {
        let out = falling.apply(pressure);
        assert!((0.0..=1.0).contains(&out), "{out} at {pressure}");
        assert!(out <= previous + 1e-6, "rises at {pressure}");
        previous = out;
    }
}

#[test]
fn a_nan_reading_counts_as_no_pressure() {
    let raised = curve(&[(0.0, 0.2), (1.0, 0.8)]);
    assert_eq!(raised.apply(f32::NAN), 0.2);
}

#[test]
fn a_flat_run_stays_flat() {
    let plateau = curve(&[(0.0, 0.0), (0.4, 0.5), (0.6, 0.5), (1.0, 1.0)]);
    for pressure in grid().filter(|p| (0.4..=0.6).contains(p)) {
        assert!((plateau.apply(pressure) - 0.5).abs() < 1e-6);
    }
}

#[test]
fn malformed_points_are_refused() {
    let refused: [&[(f32, f32)]; 9] = [
        &[(0.0, 0.0)],
        &[(0.1, 0.0), (1.0, 1.0)],
        &[(0.0, 0.0), (0.9, 1.0)],
        &[(0.0, 0.0), (0.6, 0.5), (0.4, 0.6), (1.0, 1.0)],
        &[(0.0, 0.0), (0.5, 0.5), (0.51, 0.6), (1.0, 1.0)],
        &[(0.0, 0.0), (0.5, 1.5), (1.0, 1.0)],
        &[(0.0, 0.0), (0.5, f32::NAN), (1.0, 1.0)],
        &[(0.0, -0.1), (1.0, 1.0)],
        &[
            (0.0, 0.0),
            (0.1, 0.1),
            (0.2, 0.2),
            (0.3, 0.3),
            (0.4, 0.4),
            (0.5, 0.5),
            (0.6, 0.6),
            (0.7, 0.7),
            (1.0, 1.0),
        ],
    ];
    for points in refused {
        assert_eq!(PressureCurve::from_points(points), None, "{points:?}");
    }
}

#[test]
fn up_to_eight_points_spaced_by_the_minimum_gap_are_accepted() {
    let mut points: Vec<(f32, f32)> = (0..MAX_POINTS - 1)
        .map(|i| (i as f32 * MIN_GAP, 0.0))
        .collect();
    points.push((1.0, 1.0));
    assert!(PressureCurve::from_points(&points).is_some());
}

#[test]
fn the_curve_is_kept_as_a_list_of_point_pairs() {
    let soft = curve(&[(0.0, 0.1), (0.5, 0.75), (1.0, 1.0)]);
    let value = serde_json::to_value(soft).unwrap();
    assert_eq!(
        value,
        serde_json::json!([[0.0, 0.1_f32], [0.5, 0.75], [1.0, 1.0]])
    );
    assert_eq!(PressureCurve::from_json(&value), Some(soft));
}

#[test]
fn json_that_is_not_a_valid_curve_reads_as_none() {
    for text in [
        r#""soft""#,
        "[[0, 0]]",
        "[[0, 0], [0.5]]",
        "[[0, 0], [0.9, 1]]",
    ] {
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(PressureCurve::from_json(&value), None, "{text}");
    }
}
