use cadmium_engine::stroke::*;

fn geometry() -> BrushGeometry {
    BrushGeometry {
        max_diameter_px: 20.0,
        min_diameter_ratio: 1.0,
        spacing: 0.05,
        pressure_curve: PressureCurve::default(),
    }
}

fn sample(x: f64, t: u32) -> StrokeSample {
    StrokeSample {
        x,
        y: 0.0,
        pressure: 1.0,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms: t,
    }
}

#[test]
fn vertical_pen_has_right_angle_altitude() {
    let a = pen_angles(0.0, 0.0);
    assert!((a.altitude_rad - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
}

#[test]
fn tilt_uses_tangent_composition_not_vector_addition() {
    let a = pen_angles(45.0, 45.0);
    let expected = (1.0f32 / 2.0f32.sqrt()).atan();
    assert!((a.altitude_rad - expected).abs() < 1e-4);
    assert!((a.azimuth_rad - std::f32::consts::FRAC_PI_4).abs() < 1e-4);
}

#[test]
fn dabs_are_spaced_by_at_least_one_pixel_along_the_arc() {
    let mut b = StrokeBuilder::new(geometry());
    let mut dabs = b.push(sample(0.0, 0));
    dabs.extend(b.push(sample(10.0, 10)));
    assert_eq!(dabs.len(), 11);
    for w in dabs.windows(2) {
        assert!(((w[1].x - w[0].x) - 1.0).abs() < 1e-4);
    }
}

#[test]
fn spacing_remainder_carries_across_samples() {
    let mut b = StrokeBuilder::new(geometry());
    let mut dabs = b.push(sample(0.0, 0));
    dabs.extend(b.push(sample(0.5, 1)));
    dabs.extend(b.push(sample(1.0, 2)));
    assert_eq!(dabs.len(), 2);
    assert!((dabs[1].x - 1.0).abs() < 1e-4);
}

#[test]
fn light_pressure_makes_smaller_dabs() {
    let g = BrushGeometry {
        min_diameter_ratio: 0.2,
        ..geometry()
    };
    let mut b = StrokeBuilder::new(g);
    let soft = b.push(StrokeSample {
        pressure: 0.1,
        ..sample(0.0, 0)
    })[0];
    let mut b = StrokeBuilder::new(g);
    let hard = b.push(sample(0.0, 0))[0];
    assert!(soft.radius < hard.radius);
}

#[test]
fn default_pressure_curve_is_linear() {
    let curve = PressureCurve::default();
    assert_eq!(curve.apply(0.3), 0.3);
    assert_eq!(curve.apply(1.7), 1.0);
    assert_eq!(curve.apply(-0.2), 0.0);
}

#[test]
fn dab_carries_segment_direction_and_speed() {
    let mut b = StrokeBuilder::new(geometry());
    b.push(StrokeSample {
        y: 0.0,
        ..sample(0.0, 0)
    });
    let dabs = b.push(StrokeSample {
        y: 4.0,
        ..sample(0.0, 2)
    });
    assert_eq!(dabs.len(), 4);
    let last = dabs[3];
    assert!((last.direction - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    assert!((last.speed - 2.0).abs() < 1e-5);
    assert!((last.y - 4.0).abs() < 1e-4);
}

#[test]
fn simultaneous_samples_use_one_millisecond_for_speed() {
    let mut b = StrokeBuilder::new(geometry());
    b.push(sample(0.0, 5));
    let dabs = b.push(sample(3.0, 5));
    assert!((dabs[0].speed - 3.0).abs() < 1e-5);
}

#[test]
fn finish_adds_nothing_after_dabs_and_resets_the_stroke() {
    let mut b = StrokeBuilder::new(geometry());
    b.push(sample(0.0, 0));
    b.push(sample(0.5, 1));
    assert!(b.finish().is_empty());
    let restarted = b.push(sample(100.0, 2));
    assert_eq!(restarted.len(), 1);
    assert_eq!(restarted[0].x, 100.0);
}

#[test]
fn finish_without_samples_emits_nothing() {
    let mut b = StrokeBuilder::new(geometry());
    assert!(b.finish().is_empty());
}

fn default_geometry() -> BrushGeometry {
    BrushGeometry {
        max_diameter_px: 40.0,
        min_diameter_ratio: 0.3,
        spacing: 0.05,
        pressure_curve: PressureCurve::default(),
    }
}

fn pen(x: f64, y: f64, pressure: f32, tilt_x_deg: f32, time_ms: u32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure,
        tilt_x_deg,
        tilt_y_deg: 0.0,
        time_ms,
    }
}

#[test]
fn dabs_never_land_behind_the_segment_start_when_pressure_drops() {
    let mut b = StrokeBuilder::new(default_geometry());
    let mut dabs = b.push(pen(0.0, 0.0, 1.0, 0.0, 0));
    dabs.extend(b.push(pen(1.9, 0.0, 1.0, 0.0, 1)));
    dabs.extend(b.push(pen(1.9, 0.01, 0.3, 0.0, 2)));
    dabs.extend(b.push(pen(1.9, 0.02, 0.0, 10.0, 3)));
    let most_tilted = pen_angles(10.0, 0.0).altitude_rad;
    for dab in &dabs {
        assert!((0.0..=1.0).contains(&dab.pressure), "{dab:?}");
        assert!(dab.altitude >= most_tilted - 1e-5, "{dab:?}");
        assert!(
            dab.altitude <= std::f32::consts::FRAC_PI_2 + 1e-5,
            "{dab:?}"
        );
        assert!(dab.azimuth.abs() < 1e-5, "{dab:?}");
        let on_first_leg = dab.y.abs() < 1e-6 && (-1e-5..=1.9 + 1e-5).contains(&dab.x);
        let on_vertical_legs = (dab.x - 1.9).abs() < 1e-5 && (-1e-6..=0.02 + 1e-6).contains(&dab.y);
        assert!(on_first_leg || on_vertical_legs, "{dab:?}");
    }
}

#[test]
fn non_finite_samples_are_dropped_without_dabs() {
    let mut b = StrokeBuilder::new(geometry());
    b.push(sample(0.0, 0));
    assert!(b.push(sample(f64::INFINITY, 1)).is_empty());
    assert!(b.push(sample(f64::NAN, 2)).is_empty());
    assert!(
        b.push(StrokeSample {
            pressure: f32::NAN,
            ..sample(5.0, 3)
        })
        .is_empty()
    );
    let resumed = b.push(sample(2.0, 4));
    assert_eq!(resumed.len(), 2);
    assert!((resumed[1].x - 2.0).abs() < 1e-4);
}

#[test]
fn stroke_starting_on_a_non_finite_sample_emits_nothing() {
    let mut b = StrokeBuilder::new(geometry());
    assert!(b.push(sample(f64::NAN, 0)).is_empty());
    assert_eq!(b.push(sample(3.0, 1)).len(), 1);
}

#[test]
fn huge_jump_is_capped_at_the_dab_budget_per_segment() {
    let mut b = StrokeBuilder::new(default_geometry());
    b.push(sample(0.0, 0));
    let dabs = b.push(sample(1e6, 1));
    assert!(dabs.len() <= 8192, "{}", dabs.len());
    assert!(dabs.len() > 4000, "{}", dabs.len());
    assert!((dabs.last().unwrap().x - 1e6).abs() < 1.0);
}

#[test]
fn extreme_tilt_is_clamped_short_of_horizontal() {
    let clamped = pen_angles(89.0, 0.0);
    assert_eq!(pen_angles(90.0, 0.0), clamped);
    assert_eq!(pen_angles(-120.0, 0.0), pen_angles(-89.0, 0.0));
    assert!(clamped.altitude_rad > 0.0);
}

#[test]
fn first_dab_points_along_the_pen_azimuth_and_is_still() {
    let mut b = StrokeBuilder::new(geometry());
    let first = b.push(StrokeSample {
        tilt_x_deg: 0.0,
        tilt_y_deg: 30.0,
        ..sample(0.0, 0)
    })[0];
    assert!((first.direction - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    assert_eq!(first.speed, 0.0);
}

#[test]
fn dab_pressure_follows_the_pressure_curve() {
    let g = BrushGeometry {
        pressure_curve: PressureCurve {
            gamma: 2.0,
            ..PressureCurve::default()
        },
        ..default_geometry()
    };
    let mut b = StrokeBuilder::new(g);
    let dab = b.push(StrokeSample {
        pressure: 0.5,
        ..sample(0.0, 0)
    })[0];
    assert!((dab.pressure - 0.25).abs() < 1e-6);
    assert!((dab.radius * 2.0 - g.diameter(0.5)).abs() < 1e-5);
}

#[test]
fn diameter_spans_min_ratio_to_max_through_the_curve() {
    let g = default_geometry();
    assert!((g.diameter(0.0) - 12.0).abs() < 1e-5);
    assert!((g.diameter(1.0) - 40.0).abs() < 1e-5);
    assert!((g.diameter(0.5) - 26.0).abs() < 1e-5);
}
