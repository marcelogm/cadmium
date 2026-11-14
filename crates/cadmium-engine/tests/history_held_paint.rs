pub mod support;

use cadmium_engine::{
    gpu::GpuContext,
    pigments::{MAX_SLOTS, Mixture},
    session::PaintSession,
};
use support::{session, stroke, without_wgpu_errors};

const BLUE: usize = 0;
const DIP_THICKNESS_UM: f32 = 40.0;
const HALF_DIP: f32 = 0.5;

fn blue_of(volume_um: f32) -> Mixture {
    let mut fractions = [0.0; MAX_SLOTS];
    fractions[BLUE] = 1.0;
    Mixture {
        volume_um,
        fractions,
    }
}

fn load(gpu: &GpuContext, s: &mut PaintSession, slot: usize) {
    s.brush.recipe = vec![(slot, 1.0)];
    without_wgpu_errors(gpu, || s.reload_brush(gpu));
}

fn held(gpu: &GpuContext, s: &PaintSession) -> (Vec<f32>, f64, f64) {
    let totals = s.engine.brush_totals(gpu).unwrap();
    (
        s.engine.tip_volumes_um(gpu).unwrap(),
        totals.volume,
        s.engine.knife_totals(gpu).unwrap().volume,
    )
}

#[test]
fn held_paint_restored_after_a_stroke_is_the_paint_held_before_it() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    let before = held(&gpu, &s);
    let captured = s.engine.capture_held_paint(&gpu).unwrap();
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    assert!(held(&gpu, &s) != before);
    without_wgpu_errors(&gpu, || s.engine.restore_held_paint(&gpu, &captured));
    assert!(held(&gpu, &s) == before);
}

#[test]
fn a_capture_holds_only_the_part_of_the_tip_the_brush_uses() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.geometry.max_diameter_px = 40.0;
    s.resize_tip();
    let small = s.engine.capture_held_paint(&gpu).unwrap().bytes();
    s.brush.geometry.max_diameter_px = 400.0;
    s.resize_tip();
    let large = s.engine.capture_held_paint(&gpu).unwrap().bytes();
    assert!(small * 4 < large, "{small} vs {large}");
}

#[test]
fn loading_cleaning_and_resizing_each_start_a_new_load_epoch_and_a_stroke_does_not() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let fresh = s.engine.load_epoch();
    load(&gpu, &mut s, BLUE);
    let loaded = s.engine.load_epoch();
    s.clean_brush(&gpu);
    let cleaned = s.engine.load_epoch();
    s.brush.geometry.max_diameter_px = 90.0;
    s.resize_tip();
    let resized = s.engine.load_epoch();
    assert!(fresh < loaded && loaded < cleaned && cleaned < resized);
    s.resize_tip();
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    assert_eq!(s.engine.load_epoch(), resized);
}

#[test]
fn every_dip_and_knife_cleaning_starts_a_new_load_epoch_and_an_empty_dip_or_a_restore_does_not() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let paint = blue_of(DIP_THICKNESS_UM);
    let mut epochs = vec![s.engine.load_epoch()];
    without_wgpu_errors(&gpu, || {
        s.engine.dip(&gpu, &paint, HALF_DIP);
        epochs.push(s.engine.load_epoch());
        s.engine.restore_reserve(&gpu, &paint);
        epochs.push(s.engine.load_epoch());
        s.engine.dip_knife(&gpu, &paint, HALF_DIP).unwrap();
        epochs.push(s.engine.load_epoch());
        s.engine.clean_knife(&gpu);
        epochs.push(s.engine.load_epoch());
    });
    assert!(epochs.is_sorted_by(|a, b| a < b), "{epochs:?}");
    let settled = s.engine.load_epoch();
    let captured = s.engine.capture_held_paint(&gpu).unwrap();
    without_wgpu_errors(&gpu, || {
        s.engine.dip(&gpu, &blue_of(0.0), HALF_DIP);
        s.engine.restore_reserve(&gpu, &blue_of(0.0));
        s.engine.dip_knife(&gpu, &paint, 0.0).unwrap();
        s.engine.restore_held_paint(&gpu, &captured);
    });
    assert_eq!(s.engine.load_epoch(), settled);
}
