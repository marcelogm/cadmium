use std::sync::mpsc;

use oleo_engine::{canvas::*, gpu::GpuContext, pigments::PigmentDb};

fn spec(primer: Primer, weave: Weave) -> CanvasSpec {
    CanvasSpec {
        width: 256,
        height: 128,
        primer,
        weave,
        pigment_slots: vec!["titanium_white".into(), "ultramarine_blue".into()],
    }
}

#[test]
fn new_canvas_has_no_wet_paint() {
    let gpu = GpuContext::new().unwrap();
    let c = Canvas::new(
        &gpu,
        &PigmentDb::builtin(),
        spec(Primer::WhiteGesso, Weave::Smooth),
    );
    assert_eq!(c.wet_totals(&gpu), PaintTotals::default());
}

#[test]
fn white_gesso_primer_reflects_like_titanium_white() {
    let gpu = GpuContext::new().unwrap();
    let db = PigmentDb::builtin();
    let c = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Smooth));
    let refl = c.read_f32(&gpu, StateTexture::DryRefl0);
    let w = db.get("titanium_white").unwrap();
    let expected = oleo_engine::spectral::km_infinite(w.absorption[0], w.scattering[0]);
    assert!((refl[0] - expected).abs() < 2e-3);
}

#[test]
fn smooth_weave_is_flat_and_linen_is_not() {
    let gpu = GpuContext::new().unwrap();
    let db = PigmentDb::builtin();
    let flat = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Smooth))
        .read_f32(&gpu, StateTexture::DryHeight);
    assert!(flat.iter().all(|h| *h == 0.0));
    let linen = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Linen))
        .read_f32(&gpu, StateTexture::DryHeight);
    let max = linen.iter().cloned().fold(0.0, f32::max);
    assert!(max > 3.0 && max <= 12.0);
}

#[test]
fn write_then_read_roundtrips_half_floats() {
    let gpu = GpuContext::new().unwrap();
    let c = Canvas::new(
        &gpu,
        &PigmentDb::builtin(),
        spec(Primer::WhiteGesso, Weave::Smooth),
    );
    let data: Vec<f32> = (0..256 * 128 * 4).map(|i| (i % 7) as f32 * 0.125).collect();
    c.write_f32(&gpu, StateTexture::WetPig0, &data);
    assert_eq!(c.read_f32(&gpu, StateTexture::WetPig0), data);
}

#[test]
fn dirty_rect_marks_every_touched_tile_once() {
    let mut d = DirtyTiles::new(256, 128);
    d.mark_rect(60.0, 10.0, 70.0, 70.0);
    assert_eq!(
        d.take(),
        vec![
            TileCoord { x: 0, y: 0 },
            TileCoord { x: 0, y: 1 },
            TileCoord { x: 1, y: 0 },
            TileCoord { x: 1, y: 1 }
        ]
    );
    assert!(d.is_empty());
}

#[test]
fn cotton_weave_stays_within_its_amplitude() {
    let gpu = GpuContext::new().unwrap();
    let cotton = Canvas::new(
        &gpu,
        &PigmentDb::builtin(),
        spec(Primer::WhiteGesso, Weave::Cotton),
    )
    .read_f32(&gpu, StateTexture::DryHeight);
    assert!(cotton.iter().all(|h| (0.0..=6.0).contains(h)));
    assert!(cotton.iter().cloned().fold(0.0, f32::max) > 1.5);
}

#[test]
fn sienna_imprimatura_is_warm_and_darker_than_gesso() {
    let gpu = GpuContext::new().unwrap();
    let db = PigmentDb::builtin();
    let gesso = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Smooth));
    let sienna = Canvas::new(&gpu, &db, spec(Primer::SiennaImprimatura, Weave::Smooth));
    let blue_band = sienna.read_f32(&gpu, StateTexture::DryRefl0)[0];
    let red_band = sienna.read_f32(&gpu, StateTexture::DryRefl1)[3];
    let gesso_blue_band = gesso.read_f32(&gpu, StateTexture::DryRefl0)[0];
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
    );
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
    );
    for which in [
        StateTexture::WetPig0,
        StateTexture::WetPig1,
        StateTexture::WetVol,
        StateTexture::DryTopPig0,
        StateTexture::DryTopPig1,
        StateTexture::Display,
    ] {
        assert!(
            c.read_f32(&gpu, which).iter().all(|v| *v == 0.0),
            "{which:?}"
        );
    }
}

#[test]
fn raw_bytes_roundtrip_with_unpadded_rows() {
    let gpu = GpuContext::new().unwrap();
    let mut odd = spec(Primer::WhiteGesso, Weave::Smooth);
    odd.width = 37;
    odd.height = 5;
    let c = Canvas::new(&gpu, &PigmentDb::builtin(), odd);
    let bytes: Vec<u8> = (0..37 * 5 * 4).map(|i| (i % 251) as u8).collect();
    c.write_bytes(&gpu, StateTexture::DryHeight, &bytes);
    assert_eq!(c.read_bytes(&gpu, StateTexture::DryHeight), bytes);
    assert_eq!(c.read_bytes(&gpu, StateTexture::DryRefl0).len(), 37 * 5 * 8);
}

#[test]
fn wet_totals_weigh_concentrations_by_volume() {
    let gpu = GpuContext::new().unwrap();
    let mut small = spec(Primer::WhiteGesso, Weave::Smooth);
    small.width = 2;
    small.height = 1;
    let c = Canvas::new(&gpu, &PigmentDb::builtin(), small);
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
    let totals = c.wet_totals(&gpu);
    assert_eq!(totals.volume, 40.0);
    assert_eq!(totals.pigment, [12.5, 2.5, 0.0, 0.0, 0.0, 0.0, 0.0, 16.25]);
}

#[test]
fn state_texture_formats_match_the_layout() {
    for which in StateTexture::ALL {
        let (format, channels) = match which {
            StateTexture::WetVol | StateTexture::DryHeight => (wgpu::TextureFormat::R32Float, 1),
            _ => (wgpu::TextureFormat::Rgba16Float, 4),
        };
        assert_eq!(which.format(), format);
        assert_eq!(which.channels(), channels);
    }
    assert!(!StateTexture::PAINT_STATE.contains(&StateTexture::Display));
    assert_eq!(StateTexture::PAINT_STATE.len(), 8);
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

#[test]
fn dirty_rect_is_clamped_to_the_canvas() {
    let mut d = DirtyTiles::new(100, 100);
    d.mark_rect(-50.0, 90.0, 500.0, 900.0);
    assert_eq!(
        d.tiles(),
        vec![TileCoord { x: 0, y: 1 }, TileCoord { x: 1, y: 1 }]
    );
    d.mark_rect(-20.0, -20.0, -10.0, -10.0);
    assert_eq!(d.tiles().len(), 2);
}

#[test]
fn rect_ending_on_a_tile_boundary_marks_the_next_tile() {
    assert_eq!(
        tiles_in_rect(256, 128, 0.0, 0.0, 64.0, 64.0),
        vec![
            TileCoord { x: 0, y: 0 },
            TileCoord { x: 0, y: 1 },
            TileCoord { x: 1, y: 0 },
            TileCoord { x: 1, y: 1 }
        ]
    );
}

#[test]
fn rect_ending_inside_the_last_pixel_of_a_tile_stays_in_that_tile() {
    assert_eq!(
        tiles_in_rect(256, 128, 0.0, 0.0, 63.9, 63.9),
        vec![TileCoord { x: 0, y: 0 }]
    );
}

#[test]
fn rect_starting_past_the_last_pixel_marks_nothing() {
    assert!(tiles_in_rect(100, 100, 100.0, 0.0, 120.0, 10.0).is_empty());
    assert_eq!(
        tiles_in_rect(100, 100, 99.5, 0.0, 120.0, 10.0),
        vec![TileCoord { x: 1, y: 0 }]
    );
}

#[test]
fn point_rect_marks_its_tile() {
    assert_eq!(
        tiles_in_rect(256, 128, 100.0, 100.0, 100.0, 100.0),
        vec![TileCoord { x: 1, y: 1 }]
    );
}

#[test]
fn mark_all_covers_partial_edge_tiles() {
    let mut d = DirtyTiles::new(130, 65);
    d.mark_all();
    assert_eq!(d.tiles().len(), 3 * 2);
    d.clear();
    assert!(d.is_empty());
}

fn validated(edit: impl FnOnce(&mut CanvasSpec)) -> Result<(), String> {
    let mut s = spec(Primer::WhiteGesso, Weave::Linen);
    edit(&mut s);
    s.validate(&PigmentDb::builtin())
}

#[test]
fn spec_validation_accepts_a_normal_canvas() {
    assert!(validated(|_| {}).is_ok());
    assert!(
        validated(|s| {
            s.width = MAX_CANVAS_SIZE;
            s.height = 1;
        })
        .is_ok()
    );
}

#[test]
fn spec_validation_rejects_out_of_range_dimensions() {
    assert!(validated(|s| s.width = 0).is_err());
    assert!(validated(|s| s.height = 0).is_err());
    assert!(validated(|s| s.width = MAX_CANVAS_SIZE + 1).is_err());
    assert!(validated(|s| s.height = MAX_CANVAS_SIZE + 1).is_err());
}

#[test]
fn spec_validation_rejects_empty_or_overfull_slot_tables() {
    assert!(validated(|s| s.pigment_slots.clear()).is_err());
    assert!(
        validated(|s| {
            s.pigment_slots = PigmentDb::builtin()
                .all()
                .iter()
                .take(9)
                .map(|p| p.id.to_string())
                .collect()
        })
        .is_err()
    );
}

#[test]
fn spec_validation_rejects_unknown_and_duplicate_pigments() {
    assert!(validated(|s| s.pigment_slots.push("mauveine".into())).is_err());
    assert!(validated(|s| s.pigment_slots.push("titanium_white".into())).is_err());
}

#[test]
fn spec_survives_a_json_roundtrip() {
    let original = spec(Primer::SiennaImprimatura, Weave::Cotton);
    let json = serde_json::to_string(&original).unwrap();
    assert_eq!(serde_json::from_str::<CanvasSpec>(&json).unwrap(), original);
}

fn read_slot_table(gpu: &GpuContext, canvas: &Canvas) -> Vec<u8> {
    let table = canvas.slot_table();
    let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("slot_table_readback"),
        size: table.size(),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_buffer_to_buffer(table, 0, &staging, 0, table.size());
    gpu.queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    receiver.recv().unwrap().unwrap();
    slice.get_mapped_range().unwrap().to_vec()
}

#[test]
fn adding_a_slot_grows_the_palette_and_rewrites_the_slot_table() {
    let gpu = GpuContext::new().unwrap();
    let db = PigmentDb::builtin();
    let mut c = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Smooth));
    assert_eq!(c.add_slot(&gpu, &db, "cadmium_red"), Ok(2));
    assert_eq!(
        c.spec.pigment_slots,
        ["titanium_white", "ultramarine_blue", "cadmium_red"]
    );
    let expected = PigmentSlotsUniform::new(&db, &c.spec.pigment_slots);
    assert_eq!(read_slot_table(&gpu, &c), bytemuck::bytes_of(&expected));
    assert_eq!(expected.count, 3);
}

#[test]
fn adding_an_unknown_duplicate_or_ninth_slot_is_refused() {
    let gpu = GpuContext::new().unwrap();
    let db = PigmentDb::builtin();
    let mut c = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Smooth));
    assert!(c.add_slot(&gpu, &db, "unobtainium").is_err());
    assert!(c.add_slot(&gpu, &db, "ultramarine_blue").is_err());
    for id in [
        "cadmium_yellow",
        "cadmium_red",
        "ivory_black",
        "burnt_umber",
        "phthalo_blue",
        "yellow_ochre",
    ] {
        assert!(c.add_slot(&gpu, &db, id).is_ok(), "{id}");
    }
    assert!(c.add_slot(&gpu, &db, "burnt_sienna").is_err());
    assert_eq!(c.spec.pigment_slots.len(), 8);
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
