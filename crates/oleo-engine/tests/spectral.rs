use oleo_engine::spectral::*;

#[test]
fn perfect_reflector_is_display_white() {
    let rgb = bands_to_linear_srgb(&[1.0; BANDS]);
    for c in rgb {
        assert!((c - 1.0).abs() < 2e-3, "{rgb:?}");
    }
}

#[test]
fn thick_layer_converges_to_infinite_reflectance() {
    let layer = km_layer(0.01, 0.4, 10_000.0);
    assert!((layer.reflectance - km_infinite(0.01, 0.4)).abs() < 1e-4);
    assert!(layer.transmittance < 1e-6);
}

#[test]
fn splitting_a_layer_in_two_gives_the_same_reflectance_over_a_background() {
    let (k, s, below) = (0.05, 0.2, 0.7);
    let whole = composite_over(&km_layer(k, s, 30.0), below);
    let lower = composite_over(&km_layer(k, s, 15.0), below);
    let split = composite_over(&km_layer(k, s, 15.0), lower);
    assert!((whole - split).abs() < 1e-4);
}

#[test]
fn non_absorbing_layer_has_no_nan() {
    let layer = km_layer(0.0, 0.3, 10.0);
    assert!(layer.reflectance.is_finite() && layer.transmittance.is_finite());
    assert!((layer.reflectance + layer.transmittance - 1.0).abs() < 1e-5);
}

#[test]
fn non_scattering_layer_is_beer_lambert() {
    let layer = km_layer(0.1, 0.0, 5.0);
    assert!(layer.reflectance.abs() < 1e-6);
    assert!((layer.transmittance - (-0.5f32).exp()).abs() < 1e-4);
}

#[test]
fn gamut_compression_keeps_values_in_unit_range() {
    let out = compress_gamut([-0.2, 0.5, 1.4]);
    for c in out {
        assert!((0.0..=1.0).contains(&c), "{out:?}");
    }
}

#[test]
fn srgb_encoding_matches_reference_points() {
    assert_eq!(linear_to_srgb8(0.0), 0);
    assert_eq!(linear_to_srgb8(1.0), 255);
    assert_eq!(linear_to_srgb8(0.215_861), 128);
}

#[test]
fn reflectance_srgb8_shows_a_perfect_reflector_as_near_white_and_black_as_black() {
    let white = reflectance_srgb8(&[1.0; BANDS]);
    assert!(white.iter().all(|channel| *channel > 240), "{white:?}");
    assert_eq!(reflectance_srgb8(&[0.0; BANDS]), [0, 0, 0]);
}
