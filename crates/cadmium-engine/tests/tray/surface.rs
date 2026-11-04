use cadmium_engine::{
    canvas::{Primer, StateTexture, Weave, primer_reflectance},
    gpu::GpuContext,
    pigments::PigmentDb,
    surface::{Surface, TRAY_HEIGHT, TRAY_WIDTH, tray_spec},
};

use crate::{session, spec};

#[test]
fn a_new_session_has_a_clean_tray_with_the_painting_pigments() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu);
    assert_eq!(s.tray.spec, tray_spec(&s.canvas.spec.pigment_slots));
    assert_eq!(
        (s.tray.spec.width, s.tray.spec.height),
        (TRAY_WIDTH, TRAY_HEIGHT)
    );
    assert_eq!(s.tray.spec.weave, Weave::Smooth);
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn the_tray_primer_is_a_flat_neutral_grey() {
    let db = PigmentDb::builtin();
    let bands = primer_reflectance(&db, Primer::Tray);
    assert!(bands.iter().all(|band| (*band - 0.75).abs() < 1e-6));
}

#[test]
fn a_pigment_added_to_the_painting_reaches_the_tray() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let slot = s.add_slot(&gpu, "titanium_white").unwrap();
    assert_eq!(slot, 2);
    assert_eq!(s.tray.spec.pigment_slots, s.canvas.spec.pigment_slots);
}

#[test]
fn surfaces_are_reached_by_name() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu);
    assert_eq!(s.canvas_of(Surface::Painting).spec, s.canvas.spec);
    assert_eq!(s.canvas_of(Surface::Tray).spec, s.tray.spec);
}

#[test]
fn replacing_the_document_starts_a_clean_tray_with_the_new_pigments() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let texels = (TRAY_WIDTH * TRAY_HEIGHT) as usize;
    s.tray
        .write_f32(&gpu, StateTexture::WetVol, &vec![20.0; texels]);
    assert!(s.tray.wet_totals(&gpu).unwrap().volume > 0.0);
    let mut other = spec();
    other.pigment_slots = vec!["burnt_sienna".into()];
    s.replace_document(&gpu, other.clone()).unwrap();
    assert_eq!(s.tray.spec, tray_spec(&other.pigment_slots));
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn the_tray_always_carries_the_painting_pigments() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.add_slot(&gpu, "titanium_white").unwrap();
    assert_eq!(s.tray.spec.pigment_slots, s.canvas.spec.pigment_slots);
    let mut other = spec();
    other.pigment_slots = vec!["burnt_sienna".into(), "cadmium_red".into()];
    s.replace_document(&gpu, other).unwrap();
    assert_eq!(s.tray.spec.pigment_slots, s.canvas.spec.pigment_slots);
    s.add_slot(&gpu, "titanium_white").unwrap();
    assert_eq!(s.tray.spec.pigment_slots, s.canvas.spec.pigment_slots);
    assert!(s.add_slot(&gpu, "unobtainium").is_err());
    assert_eq!(s.tray.spec.pigment_slots, s.canvas.spec.pigment_slots);
}
