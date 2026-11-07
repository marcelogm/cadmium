use cadmium_engine::{
    gpu::GpuContext,
    pigments::Mixture,
    session::{ClearOutcome, PaintSession},
    surface::Surface,
};

use crate::{load_slot, sample, session, stroke_on};

#[test]
fn a_stroke_on_the_tray_leaves_the_painting_untouched() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Tray, 100.0);
    assert!(s.tray.wet_totals(&gpu).unwrap().volume > 0.0);
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn the_tip_dirtied_on_the_tray_carries_that_paint_to_the_painting() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 1);
    stroke_on(&gpu, &mut s, Surface::Tray, 100.0);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Tray, 100.0);
    stroke_on(&gpu, &mut s, Surface::Painting, 100.0);
    assert!(s.canvas.wet_totals(&gpu).unwrap().pigment[1] > 0.0);
}

fn assert_undoing_twice_takes_back_the_tray_then_the_painting(
    gpu: &GpuContext,
    s: &mut PaintSession,
) {
    assert!(s.undo(gpu));
    assert_eq!(s.tray.wet_totals(gpu).unwrap().volume, 0.0);
    assert!(s.canvas.wet_totals(gpu).unwrap().volume > 0.0);
    assert!(s.undo(gpu));
    assert_eq!(s.canvas.wet_totals(gpu).unwrap().volume, 0.0);
    assert_eq!(s.tray.wet_totals(gpu).unwrap().volume, 0.0);
}

#[test]
fn undo_follows_one_order_across_tray_and_painting() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Painting, 60.0);
    stroke_on(&gpu, &mut s, Surface::Tray, 60.0);
    assert_undoing_twice_takes_back_the_tray_then_the_painting(&gpu, &mut s);
    assert!(s.redo(&gpu));
    assert!(s.redo(&gpu));
    assert!(s.canvas.wet_totals(&gpu).unwrap().volume > 0.0);
    assert!(s.tray.wet_totals(&gpu).unwrap().volume > 0.0);
}

#[test]
fn drying_leaves_the_tray_wet() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Tray, 60.0);
    stroke_on(&gpu, &mut s, Surface::Painting, 60.0);
    let before = s.tray.wet_totals(&gpu).unwrap().volume;
    s.dry(&gpu);
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, before);
}

#[test]
fn the_eyedropper_reads_the_tray() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 1);
    stroke_on(&gpu, &mut s, Surface::Tray, 100.0);
    let picked: Mixture = s
        .pick_colour_on(&gpu, Surface::Tray, 200.0, 100.0)
        .unwrap()
        .unwrap();
    assert!(picked.fractions[1] > picked.fractions[0]);
    assert!(
        s.pick_colour_on(&gpu, Surface::Painting, 200.0, 100.0)
            .unwrap()
            .is_none()
    );
}

#[test]
fn clearing_the_tray_can_be_undone() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Tray, 60.0);
    let painted = s.tray.wet_totals(&gpu).unwrap().volume;
    assert_eq!(s.clear_tray(&gpu), ClearOutcome::Cleared { undoable: true });
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(s.undo(&gpu));
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, painted);
}

#[test]
fn redoing_a_clear_leaves_nothing_to_clear() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Tray, 60.0);
    s.clear_tray(&gpu);
    assert!(s.undo(&gpu));
    assert!(s.redo(&gpu));
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
    assert_eq!(s.clear_tray(&gpu), ClearOutcome::NothingWet);
}

#[test]
fn queued_dabs_land_on_the_surface_they_were_painted_on() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 1);
    s.begin_stroke_on(Surface::Tray, sample(40.0, 100.0, 0));
    for i in 1..=40 {
        s.add_samples(&[sample(40.0 + 10.0 * f64::from(i), 100.0, 8 * i)]);
    }
    stroke_on(&gpu, &mut s, Surface::Painting, 200.0);
    assert!(
        s.pick_colour_on(&gpu, Surface::Tray, 200.0, 100.0)
            .unwrap()
            .is_some()
    );
    assert!(
        s.pick_colour_on(&gpu, Surface::Painting, 200.0, 100.0)
            .unwrap()
            .is_none()
    );
    assert!(
        s.pick_colour_on(&gpu, Surface::Painting, 200.0, 200.0)
            .unwrap()
            .is_some()
    );
}
