mod scene;

use std::ops::Range;

use cadmium_engine::{
    draw_up::DrawUpRequest, engine::PaintEngine, gpu::GpuContext, session::PaintSession,
    surface::Surface,
};

use scene::*;

const UPRIGHT_TILT_DEG: f32 = 0.0;
const VOLUME_TOLERANCE: f64 = 1e-6;
const PIGMENT_TOLERANCE: f64 = 1e-3;
const FIRST_PASS_KEEPS: f64 = 0.6;
const EIGHT_PASSES_LEAVE: f64 = 0.35;
const MONOTONE_SLACK: f64 = 1.05;
const SMEARED: f32 = 0.6;
const UNSMEARED: f32 = 0.0;
const SMEAR_PASSES: usize = 3;
const STREAK_X: Range<usize> = 250..451;
const STREAK_Y: Range<usize> = 280..341;
const STAMP_Y: f64 = 560.0;
const STREAK_HIGH_A: f64 = 0.7;
const STREAK_LOW_A: f64 = 0.3;

#[test]
fn mixing_on_the_tray_conserves_volume_and_each_pigment() {
    let gpu = GpuContext::new().unwrap();
    let mut s = mixing_session(&gpu);
    two_cords(&gpu, &mut s);
    let before = tray_and_tools(&gpu, &s);
    for index in 0..6 {
        knife_pass(&gpu, &mut s, mixing_pass(index));
    }
    let after = tray_and_tools(&gpu, &s);
    assert!(face_totals(&gpu, &s).volume > 0.0);
    let volume_drift = (after.volume - before.volume) / before.volume;
    eprintln!("volume drift {volume_drift:e}");
    assert!(volume_drift.abs() <= VOLUME_TOLERANCE);
    for slot in 0..2 {
        let drift = (after.pigment[slot] - before.pigment[slot]) / before.volume;
        eprintln!("pigment {slot} drift {drift:e}");
        assert!(drift.abs() < PIGMENT_TOLERANCE);
    }
}

fn variances_over_passes(gpu: &GpuContext, s: &mut PaintSession, passes: usize) -> Vec<f64> {
    two_cords(gpu, s);
    write_tray_preview(gpu, s, "mixing_0.png");
    let mut variances = vec![mixed_region_variance(gpu, s)];
    for index in 0..passes {
        knife_pass(gpu, s, mixing_pass(index));
        write_tray_preview(gpu, s, &format!("mixing_{}.png", index + 1));
        variances.push(mixed_region_variance(gpu, s));
    }
    variances
}

#[test]
fn each_pass_mixes_the_two_colours_a_little_more() {
    let gpu = GpuContext::new().unwrap();
    let mut s = mixing_session(&gpu);
    let variances = variances_over_passes(&gpu, &mut s, 8);
    let initial = variances[0];
    let relative: Vec<f64> = variances.iter().map(|v| v / initial).collect();
    eprintln!("relative variance per pass {relative:?}");
    assert!(relative[1] >= FIRST_PASS_KEEPS);
    assert!(relative[1] < 1.0);
    for pair in relative.windows(2) {
        assert!(pair[1] <= pair[0] * MONOTONE_SLACK, "{relative:?}");
    }
    assert!(relative[8] <= EIGHT_PASSES_LEAVE);
}

/// The streaks left after passes laid flat along both cords, the blade's length
/// across them: only working the face mixes paint across the stroke.
fn streaks_with_smear(gpu: &GpuContext, smear: f32) -> f64 {
    let mut s = mixing_session(gpu);
    s.engine.params_mut().set("face_smear", smear);
    two_cords(gpu, &mut s);
    for index in 0..SMEAR_PASSES {
        let (from, to) = if index.is_multiple_of(2) {
            (200.0, 500.0)
        } else {
            (500.0, 200.0)
        };
        knife_pass(gpu, &mut s, along_x(CORD_MIDDLE_Y, from, to, LAID_TILT_DEG));
    }
    write_tray_preview(gpu, &s, &format!("smear_{smear}.png"));
    weighted_variance(&TrayPaint::read(gpu, &s), STREAK_X, STREAK_Y)
}

#[test]
fn more_smear_mixes_sooner() {
    let gpu = GpuContext::new().unwrap();
    let smeared = streaks_with_smear(&gpu, SMEARED);
    let unsmeared = streaks_with_smear(&gpu, UNSMEARED);
    eprintln!("smeared {smeared}, unsmeared {unsmeared}");
    assert!(smeared < unsmeared);
}

fn scrape_across_both_cords(gpu: &GpuContext, s: &mut PaintSession) {
    two_cords(gpu, s);
    knife_pass(
        gpu,
        s,
        along_x(CORD_MIDDLE_Y, 200.0, 500.0, UPRIGHT_TILT_DEG),
    );
}

fn face_volume_after_a_pass(gpu: &GpuContext, tilt_x_deg: f32) -> f64 {
    let mut s = mixing_session(gpu);
    two_cords(gpu, &mut s);
    knife_pass(
        gpu,
        &mut s,
        along_x(CORD_MIDDLE_Y, 200.0, 500.0, tilt_x_deg),
    );
    face_totals(gpu, &s).volume
}

fn laid_on_a_clean_patch(gpu: &GpuContext, tilt_x_deg: f32) -> f64 {
    let mut s = mixing_session(gpu);
    scrape_across_both_cords(gpu, &mut s);
    let before = TrayPaint::read(gpu, &s).volume_in(200..501, 500..620);
    knife_pass(gpu, &mut s, along_x(STAMP_Y, 200.0, 500.0, tilt_x_deg));
    TrayPaint::read(gpu, &s).volume_in(200..501, 500..620) - before
}

#[test]
fn upright_scrapes_up_and_laid_flat_spreads() {
    let gpu = GpuContext::new().unwrap();
    let upright = face_volume_after_a_pass(&gpu, UPRIGHT_TILT_DEG);
    let laid = face_volume_after_a_pass(&gpu, LAID_TILT_DEG);
    eprintln!("face after upright {upright}, after laid {laid}");
    assert!(upright > laid);
    let laid_down = laid_on_a_clean_patch(&gpu, LAID_TILT_DEG);
    let upright_down = laid_on_a_clean_patch(&gpu, UPRIGHT_TILT_DEG);
    eprintln!("laid down {laid_down}, upright down {upright_down}");
    assert!(laid_down > upright_down);
}

#[test]
fn a_loaded_face_stamps_its_streaks() {
    let gpu = GpuContext::new().unwrap();
    let mut s = mixing_session(&gpu);
    scrape_across_both_cords(&gpu, &mut s);
    knife_pass(&gpu, &mut s, along_x(STAMP_Y, 200.0, 500.0, LAID_TILT_DEG));
    write_tray_preview(&gpu, &s, "stamp.png");
    let paint = TrayPaint::read(&gpu, &s);
    let a_side = STAMP_Y as usize - 13..STAMP_Y as usize - 5;
    let b_side = STAMP_Y as usize + 5..STAMP_Y as usize + 13;
    let on_a = paint.shares_in(250..450, a_side);
    let on_b = paint.shares_in(250..450, b_side);
    assert!(!on_a.is_empty() && !on_b.is_empty());
    let (a, b) = (mean(&on_a), mean(&on_b));
    eprintln!("share of A on the A side {a}, on the B side {b}");
    assert!(a > STREAK_HIGH_A);
    assert!(b < STREAK_LOW_A);
}

#[test]
fn cleaning_the_mixing_knife_empties_its_face() {
    let gpu = GpuContext::new().unwrap();
    let mut s = mixing_session(&gpu);
    scrape_across_both_cords(&gpu, &mut s);
    assert!(face_totals(&gpu, &s).volume > 0.0);
    s.clean_brush(&gpu);
    assert_eq!(face_totals(&gpu, &s).volume, 0.0);
    assert_eq!(s.read_brush(&gpu).unwrap().face.volume, 0.0);
}

#[test]
fn the_mixing_knife_loads_only_by_working_the_tray() {
    let gpu = GpuContext::new().unwrap();
    let mut s = mixing_session(&gpu);
    s.clean_brush(&gpu);
    let brush_before = s.engine.brush_totals(&gpu).unwrap().volume;
    let mixture = s.brush.reservoir_mixture(2);
    s.dip(&gpu, &mixture, 1.0).unwrap();
    two_cords(&gpu, &mut s);
    s.begin_hold(&gpu, Surface::Tray);
    let drawn = s
        .draw_up(
            &gpu,
            DrawUpRequest {
                center: [300.0, CORD_A_Y],
                radius: 12.0,
                pressure: 1.0,
                seconds: 0.5,
            },
        )
        .unwrap();
    s.end_hold();
    assert_eq!(drawn.taken.volume, 0.0);
    assert_eq!(face_totals(&gpu, &s).volume, 0.0);
    assert_eq!(s.engine.knife_totals(&gpu).unwrap().volume, 0.0);
    assert_eq!(s.engine.brush_totals(&gpu).unwrap().volume, brush_before);
}
