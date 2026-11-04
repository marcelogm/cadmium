use cadmium_engine::{canvas::*, gpu::GpuContext, pigments::PigmentDb};

use crate::spec;

#[test]
fn new_canvas_has_no_wet_paint() {
    let gpu = GpuContext::new().unwrap();
    let c = Canvas::new(
        &gpu,
        &PigmentDb::builtin(),
        spec(Primer::WhiteGesso, Weave::Smooth),
    )
    .unwrap();
    assert_eq!(c.wet_totals(&gpu).unwrap(), PaintTotals::default());
}

#[test]
fn white_gesso_primer_reflects_like_titanium_white() {
    let gpu = GpuContext::new().unwrap();
    let db = PigmentDb::builtin();
    let c = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Smooth)).unwrap();
    let refl = c.read_f32(&gpu, StateTexture::DryRefl0).unwrap();
    let w = db.get("titanium_white").unwrap();
    let expected = cadmium_engine::spectral::km_infinite(w.absorption[0], w.scattering[0]);
    assert!((refl[0] - expected).abs() < 2e-3);
}

#[test]
fn smooth_weave_is_flat_and_linen_is_not() {
    let gpu = GpuContext::new().unwrap();
    let db = PigmentDb::builtin();
    let flat = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Smooth))
        .unwrap()
        .read_f32(&gpu, StateTexture::DryHeight)
        .unwrap();
    assert!(flat.iter().all(|h| *h == 0.0));
    let linen = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Linen))
        .unwrap()
        .read_f32(&gpu, StateTexture::DryHeight)
        .unwrap();
    let max = linen.iter().cloned().fold(0.0, f32::max);
    assert!(max > 3.0 && max <= 12.0);
}

#[test]
fn cotton_weave_stays_within_its_amplitude() {
    let gpu = GpuContext::new().unwrap();
    let cotton = Canvas::new(
        &gpu,
        &PigmentDb::builtin(),
        spec(Primer::WhiteGesso, Weave::Cotton),
    )
    .unwrap()
    .read_f32(&gpu, StateTexture::DryHeight)
    .unwrap();
    assert!(cotton.iter().all(|h| (0.0..=6.0).contains(h)));
    assert!(cotton.iter().cloned().fold(0.0, f32::max) > 1.5);
}

#[test]
fn sienna_imprimatura_is_warm_and_darker_than_gesso() {
    let gpu = GpuContext::new().unwrap();
    let db = PigmentDb::builtin();
    let gesso = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Smooth)).unwrap();
    let sienna = Canvas::new(&gpu, &db, spec(Primer::SiennaImprimatura, Weave::Smooth)).unwrap();
    let blue_band = sienna.read_f32(&gpu, StateTexture::DryRefl0).unwrap()[0];
    let red_band = sienna.read_f32(&gpu, StateTexture::DryRefl1).unwrap()[3];
    let gesso_blue_band = gesso.read_f32(&gpu, StateTexture::DryRefl0).unwrap()[0];
    assert!(red_band > blue_band);
    assert!(blue_band < gesso_blue_band);
}

#[test]
fn new_canvas_starts_fully_dirty_with_no_wet_tiles() {
    let gpu = GpuContext::new().unwrap();
    let c = Canvas::new(
        &gpu,
        &PigmentDb::builtin(),
        spec(Primer::WhiteGesso, Weave::Smooth),
    )
    .unwrap();
    assert_eq!(c.dirty.tiles().len(), 4 * 2);
    assert!(c.wet_tiles.is_empty());
}

#[test]
fn new_canvas_clears_wet_dry_top_and_display_textures() {
    let gpu = GpuContext::new().unwrap();
    let c = Canvas::new(
        &gpu,
        &PigmentDb::builtin(),
        spec(Primer::SiennaImprimatura, Weave::Linen),
    )
    .unwrap();
    for which in [
        StateTexture::WetPig0,
        StateTexture::WetPig1,
        StateTexture::WetVol,
        StateTexture::DryTopPig0,
        StateTexture::DryTopPig1,
        StateTexture::Display,
    ] {
        assert!(
            c.read_f32(&gpu, which).unwrap().iter().all(|v| *v == 0.0),
            "{which:?}"
        );
    }
}

#[test]
fn gesso_swatch_is_near_white_and_imprimatura_is_warm() {
    let db = PigmentDb::builtin();
    let gesso = primer_srgb8(&db, Primer::WhiteGesso);
    assert!(gesso.iter().all(|channel| *channel > 200), "{gesso:?}");
    let [red, green, blue] = primer_srgb8(&db, Primer::SiennaImprimatura);
    assert!(red > green && green > blue, "{:?}", [red, green, blue]);
    assert!(red < gesso[0] || blue < gesso[2]);
}
