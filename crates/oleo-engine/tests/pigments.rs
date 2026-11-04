use oleo_engine::pigments::*;

#[test]
fn has_twelve_pigments_with_unique_ids() {
    let db = PigmentDb::builtin();
    assert_eq!(db.all().len(), 12);
    let mut ids: Vec<_> = db.all().iter().map(|p| p.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 12);
}

#[test]
fn burnt_sienna_and_burnt_umber_share_a_colour_index_but_not_an_id() {
    let db = PigmentDb::builtin();
    assert_eq!(
        db.get("burnt_sienna").unwrap().colour_index,
        db.get("burnt_umber").unwrap().colour_index
    );
    assert_ne!(
        db.get("burnt_sienna").unwrap().id,
        db.get("burnt_umber").unwrap().id
    );
}

#[test]
fn ultramarine_and_cadmium_yellow_mix_to_green() {
    let db = PigmentDb::builtin();
    let slots = vec!["ultramarine_blue".to_string(), "cadmium_yellow".to_string()];
    let mut f = [0.0; MAX_SLOTS];
    f[0] = 0.5;
    f[1] = 0.5;
    let r = mixture_reflectance(&db, &slots, &f, 1000.0, &[0.0; 8]);
    let shown: [f32; 8] = r.map(oleo_engine::spectral::saunderson_display);
    let [red, green, blue] = oleo_engine::spectral::bands_to_linear_srgb(&shown);
    assert!(green > red && green > blue, "{red} {green} {blue}");
}

#[test]
fn titanium_white_covers_black_within_one_percent_at_one_hundred_microns() {
    let db = PigmentDb::builtin();
    let white = db.get("titanium_white").unwrap();
    for b in 0..8 {
        let thin = oleo_engine::spectral::km_layer(white.absorption[b], white.scattering[b], 100.0)
            .reflectance;
        let inf = oleo_engine::spectral::km_infinite(white.absorption[b], white.scattering[b]);
        assert!((thin - inf).abs() < 0.01);
    }
}

#[test]
fn phthalo_blue_tint_stays_blue() {
    let db = PigmentDb::builtin();
    let [r, g, b] = db.tint_srgb8("phthalo_blue", 19.0);
    assert!(b > g && g > r);
}

#[test]
fn cadmium_yellow_masstone_is_yellow() {
    let [r, g, b] = PigmentDb::builtin().masstone_srgb8("cadmium_yellow");
    assert!(r > 200 && g > 150 && b < 40);
}

#[test]
fn quinacridone_glaze_over_black_is_darker_than_over_white() {
    let db = PigmentDb::builtin();
    let over_black: u32 = db
        .glaze_srgb8("quinacridone_magenta", true)
        .iter()
        .map(|&c| c as u32)
        .sum();
    let over_white: u32 = db
        .glaze_srgb8("quinacridone_magenta", false)
        .iter()
        .map(|&c| c as u32)
        .sum();
    assert!(over_black < over_white);
}
