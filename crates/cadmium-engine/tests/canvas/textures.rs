use cadmium_engine::{canvas::*, gpu::GpuContext, pigments::PigmentDb};

use crate::spec;

#[test]
fn write_then_read_roundtrips_half_floats() {
    let gpu = GpuContext::new().unwrap();
    let c = Canvas::new(
        &gpu,
        &PigmentDb::builtin(),
        spec(Primer::WhiteGesso, Weave::Smooth),
    )
    .unwrap();
    let data: Vec<f32> = (0..256 * 128 * 4).map(|i| (i % 7) as f32 * 0.125).collect();
    c.write_f32(&gpu, StateTexture::WetPig0, &data);
    assert_eq!(c.read_f32(&gpu, StateTexture::WetPig0).unwrap(), data);
}

#[test]
fn raw_bytes_roundtrip_with_unpadded_rows() {
    let gpu = GpuContext::new().unwrap();
    let mut odd = spec(Primer::WhiteGesso, Weave::Smooth);
    odd.width = 37;
    odd.height = 5;
    let c = Canvas::new(&gpu, &PigmentDb::builtin(), odd).unwrap();
    let bytes: Vec<u8> = (0..37 * 5 * 4).map(|i| (i % 251) as u8).collect();
    c.write_bytes(&gpu, StateTexture::DryHeight, &bytes);
    assert_eq!(c.read_bytes(&gpu, StateTexture::DryHeight).unwrap(), bytes);
    assert_eq!(
        c.read_bytes(&gpu, StateTexture::DryRefl0).unwrap().len(),
        37 * 5 * 8
    );
}

#[test]
fn wet_totals_weigh_concentrations_by_volume() {
    let gpu = GpuContext::new().unwrap();
    let mut small = spec(Primer::WhiteGesso, Weave::Smooth);
    small.width = 2;
    small.height = 1;
    let c = Canvas::new(&gpu, &PigmentDb::builtin(), small).unwrap();
    c.write_f32(&gpu, StateTexture::WetVol, &[10.0, 30.0]);
    c.write_f32(
        &gpu,
        StateTexture::WetPig0,
        &[0.5, 0.25, 0.0, 0.0, 0.25, 0.0, 0.0, 0.0],
    );
    c.write_f32(
        &gpu,
        StateTexture::WetPig1,
        &[0.0, 0.0, 0.0, 0.125, 0.0, 0.0, 0.0, 0.5],
    );
    let totals = c.wet_totals(&gpu).unwrap();
    assert_eq!(totals.volume, 40.0);
    assert_eq!(totals.pigment, [12.5, 2.5, 0.0, 0.0, 0.0, 0.0, 0.0, 16.25]);
}

#[test]
fn state_texture_formats_match_the_layout() {
    for which in StateTexture::ALL {
        let (format, channels) = match which {
            StateTexture::WetVol | StateTexture::DryHeight | StateTexture::Graphite => {
                (wgpu::TextureFormat::R32Float, 1)
            }
            _ => (wgpu::TextureFormat::Rgba16Float, 4),
        };
        assert_eq!(which.format(), format);
        assert_eq!(which.channels(), channels);
    }
    assert!(!StateTexture::PAINT_STATE.contains(&StateTexture::Display));
    assert_eq!(StateTexture::PAINT_STATE.len(), 9);
}

#[test]
fn graphite_is_paint_state_kept_as_one_float_per_texel() {
    assert!(StateTexture::PAINT_STATE.contains(&StateTexture::Graphite));
    assert_eq!(StateTexture::Graphite.format(), wgpu::TextureFormat::R32Float);
    assert_eq!(StateTexture::Graphite.label(), "graphite");
}

#[test]
fn half_float_conversion_handles_specials_and_rounding() {
    for value in [
        0.0,
        -0.0,
        1.0,
        -2.5,
        65504.0,
        6.103_515_6e-5,
        5.960_464_5e-8,
    ] {
        assert_eq!(half_to_f32(f32_to_half(value)), value);
    }
    assert_eq!(half_to_f32(f32_to_half(1.0e6)), f32::INFINITY);
    assert!(half_to_f32(f32_to_half(f32::NAN)).is_nan());
    assert_eq!(half_to_f32(f32_to_half(1.0 + 1.0 / 4096.0)), 1.0);
    assert_eq!(
        half_to_f32(f32_to_half(1.0 + 3.0 / 4096.0)),
        1.0 + 1.0 / 1024.0
    );
    assert_eq!(f32_to_half(1.0e-9), 0);
}
