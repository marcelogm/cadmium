pub mod support;

use cadmium_engine::{canvas::*, gpu::GpuContext, session::PaintSession, stroke::StrokeSample};
use support::{encode_frame, write_preview};

const DEFAULT_SPACING: f32 = 0.05;
const DENSE_SPACING: f32 = 0.025;
const SPACING_VOLUME_TOLERANCE: f64 = 0.05;
const BROAD_BRUSH_DIAMETER_PX: f32 = 80.0;

fn session(gpu: &GpuContext) -> PaintSession {
    PaintSession::new(
        gpu,
        CanvasSpec {
            width: 512,
            height: 256,
            primer: Primer::WhiteGesso,
            weave: Weave::Linen,
            pigment_slots: vec![
                "ultramarine_blue".into(),
                "cadmium_yellow".into(),
                "titanium_white".into(),
            ],
        },
    )
    .unwrap()
}

fn stroke(gpu: &GpuContext, s: &mut PaintSession, y: f64, from: f64, to: f64) {
    let steps = 60;
    for i in 0..=steps {
        let x = from + (to - from) * i as f64 / steps as f64;
        let sample = StrokeSample {
            x,
            y,
            pressure: 0.8,
            tilt_x_deg: 20.0,
            tilt_y_deg: 0.0,
            time_ms: i * 4,
        };
        if i == 0 {
            s.begin_stroke(gpu, sample)
        } else {
            s.add_samples(&[sample])
        }
        if i % 4 == 0 {
            encode_frame(gpu, s);
        }
    }
    s.end_stroke();
    encode_frame(gpu, s);
}

fn total(gpu: &GpuContext, s: &PaintSession) -> PaintTotals {
    let c = s.canvas.wet_totals(gpu).unwrap();
    let b = s.engine.brush_totals(gpu).unwrap();
    let mut t = c;
    t.volume += b.volume;
    for i in 0..8 {
        t.pigment[i] += b.pigment[i];
    }
    t
}

#[test]
fn a_stroke_deposits_paint_on_the_canvas() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    write_preview(&gpu, &s, "engine-brush.png");
    let wet = s.canvas.wet_totals(&gpu).unwrap();
    assert!(wet.volume > 1000.0 && wet.pigment[0] > 0.0);
}

fn wet_volume_laid_with_spacing(gpu: &GpuContext, spacing: f32) -> f64 {
    let mut s = session(gpu);
    s.brush.geometry.spacing = spacing;
    s.brush.geometry.max_diameter_px = BROAD_BRUSH_DIAMETER_PX;
    s.reload_brush(gpu);
    stroke(gpu, &mut s, 128.0, 40.0, 470.0);
    s.canvas.wet_totals(gpu).unwrap().volume
}

#[test]
fn the_laid_volume_does_not_depend_on_the_dab_spacing() {
    let gpu = GpuContext::new().unwrap();
    let sparse = wet_volume_laid_with_spacing(&gpu, DEFAULT_SPACING);
    let dense = wet_volume_laid_with_spacing(&gpu, DENSE_SPACING);
    let difference = (dense - sparse).abs() / sparse;
    eprintln!(
        "laid {sparse} µm·px at spacing {DEFAULT_SPACING}, {dense} at {DENSE_SPACING} ({difference:.4})"
    );
    assert!(difference <= SPACING_VOLUME_TOLERANCE, "{difference}");
}

#[test]
fn brush_and_canvas_together_conserve_volume_and_each_pigment() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = total(&gpu, &s);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let first = total(&gpu, &s);
    assert!(
        ((first.volume - before.volume) / before.volume).abs() < 1e-4,
        "{before:?} {first:?}"
    );
    s.brush.recipe = vec![(1, 1.0)];
    s.reload_brush(&gpu);
    let reloaded = total(&gpu, &s);
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    stroke(&gpu, &mut s, 140.0, 40.0, 470.0);
    let after = total(&gpu, &s);
    eprintln!(
        "volume drift {:e}, pigment drift {:?}",
        (after.volume - reloaded.volume) / reloaded.volume,
        (0..3)
            .map(|i| (after.pigment[i] - reloaded.pigment[i]) / reloaded.volume)
            .collect::<Vec<_>>()
    );
    assert!(
        ((after.volume - reloaded.volume) / reloaded.volume).abs() < 1e-4,
        "{before:?} {after:?}"
    );
    for i in 0..3 {
        assert!(
            ((after.pigment[i] - reloaded.pigment[i]) / reloaded.volume).abs() < 5e-3,
            "pigment {i}"
        );
    }
}

#[test]
fn half_precision_fractions_round_to_nearest_so_pigment_does_not_drift() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = total(&gpu, &s);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    stroke(&gpu, &mut s, 132.0, 470.0, 40.0);
    let after = total(&gpu, &s);
    let drift = (after.pigment[0] - before.pigment[0]) / before.volume;
    assert!(drift.abs() < 2e-4, "{drift:e}");
}

#[test]
fn painting_yellow_over_wet_blue_picks_blue_into_the_brush() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    s.brush.recipe = vec![(1, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    assert!(s.engine.brush_totals(&gpu).unwrap().pigment[0] > 0.0);
}

#[test]
fn canvas_never_gets_negative_volume() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    for k in 0..5 {
        stroke(&gpu, &mut s, 128.0 + k as f64 * 3.0, 40.0, 470.0);
    }
    assert!(
        s.canvas
            .read_f32(&gpu, StateTexture::WetVol)
            .unwrap()
            .iter()
            .all(|v| *v >= 0.0)
    );
}

#[test]
fn a_finished_frame_leaves_no_pending_work() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    assert!(s.has_pending_work());
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    assert!(!s.has_pending_work());
}

#[test]
fn a_dab_marks_its_tiles_wet_and_dirty_including_the_apron() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(&gpu, &mut e);
    gpu.queue.submit([e.finish()]);
    s.begin_stroke(
        &gpu,
        StrokeSample {
            x: 64.0 + 0.5,
            y: 100.0,
            pressure: 1.0,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            time_ms: 0,
        },
    );
    s.end_stroke();
    assert!(s.has_pending_work());
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(&gpu, &mut e);
    gpu.queue.submit([e.finish()]);
    let wet = s.canvas.wet_tiles.tiles();
    assert!(wet.contains(&TileCoord { x: 0, y: 1 }));
    assert!(wet.contains(&TileCoord { x: 1, y: 1 }));
    assert!(!wet.contains(&TileCoord { x: 3, y: 1 }));
}

#[test]
fn clean_brush_empties_tip_and_body() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    assert!(s.engine.brush_totals(&gpu).unwrap().volume > 0.0);
    s.clean_brush(&gpu);
    assert_eq!(s.engine.brush_totals(&gpu).unwrap().volume, 0.0);
}
