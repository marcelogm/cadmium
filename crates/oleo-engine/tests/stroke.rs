use oleo_engine::stroke::*;

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
