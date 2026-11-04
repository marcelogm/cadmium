use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use oleo_engine::canvas::*;
use oleo_engine::document::{self, DocumentError};
use oleo_engine::gpu::GpuContext;
use oleo_engine::render::ViewMode;
use oleo_engine::session::PaintSession;
use oleo_engine::stroke::StrokeSample;

fn sample(x: f64, y: f64, time_ms: u32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure: 1.0,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms,
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("oleo-doc-test");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn painted_session(gpu: &GpuContext) -> PaintSession {
    painted_session_with_stroke(gpu, (20.0, 60.0), (230.0, 70.0))
}

fn painted_session_with_stroke(gpu: &GpuContext, from: (f64, f64), to: (f64, f64)) -> PaintSession {
    let mut s = PaintSession::new(
        gpu,
        CanvasSpec {
            width: 256,
            height: 128,
            primer: Primer::SiennaImprimatura,
            weave: Weave::Cotton,
            pigment_slots: vec!["burnt_sienna".into(), "titanium_white".into()],
        },
    );
    s.begin_stroke(gpu, sample(from.0, from.1, 0));
    s.add_samples(&[sample(to.0, to.1, 200)]);
    s.end_stroke(gpu);
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(gpu, &mut e);
    gpu.queue.submit([e.finish()]);
    s
}

fn rewrite_entries(
    source: &Path,
    target: &Path,
    mut transform: impl FnMut(&str, Vec<u8>) -> Option<Vec<u8>>,
) {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(source).unwrap()).unwrap();
    let mut writer = zip::ZipWriter::new(std::fs::File::create(target).unwrap());
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if let Some(bytes) = transform(&name, bytes) {
            writer.start_file(name, options).unwrap();
            writer.write_all(&bytes).unwrap();
        }
    }
    writer.finish().unwrap();
}

fn rewrite_manifest(source: &Path, target: &Path, edit: impl Fn(&mut serde_json::Value)) {
    rewrite_entries(source, target, |name, bytes| {
        if name != "manifest.json" {
            return Some(bytes);
        }
        let mut manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        edit(&mut manifest);
        Some(serde_json::to_vec(&manifest).unwrap())
    });
}

#[test]
fn save_then_open_restores_every_state_texture() {
    let gpu = GpuContext::new().unwrap();
    let s = painted_session(&gpu);
    let path = scratch("t.oleo");
    document::save(&gpu, &s, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    assert_eq!(opened.canvas.spec, s.canvas.spec);
    for t in StateTexture::PAINT_STATE {
        assert_eq!(
            opened.canvas.read_bytes(&gpu, t),
            s.canvas.read_bytes(&gpu, t),
            "{t:?}"
        );
    }
}

#[test]
fn opened_document_marks_every_tile_dirty_and_only_painted_tiles_wet() {
    let gpu = GpuContext::new().unwrap();
    let s = painted_session_with_stroke(&gpu, (20.0, 20.0), (40.0, 30.0));
    let path = scratch("wet.oleo");
    document::save(&gpu, &s, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    let mut full = DirtyTiles::new(256, 128);
    full.mark_all();
    assert_eq!(opened.canvas.dirty.tiles(), full.tiles());
    let wet = opened.canvas.wet_tiles.tiles();
    assert!(!wet.is_empty());
    assert!(wet.len() < full.tiles().len());
    let volume = opened.canvas.read_f32(&gpu, StateTexture::WetVol);
    for (index, &v) in volume.iter().enumerate() {
        if v > 0.0 {
            let tile = TileCoord {
                x: (index as u32 % 256) / TILE_SIZE,
                y: (index as u32 / 256) / TILE_SIZE,
            };
            assert!(wet.contains(&tile), "{tile:?} holds paint but is not wet");
        }
    }
}

#[test]
fn drying_an_opened_document_clears_its_wet_layer() {
    let gpu = GpuContext::new().unwrap();
    let s = painted_session(&gpu);
    let path = scratch("dry.oleo");
    document::save(&gpu, &s, &path).unwrap();
    let mut opened = document::open(&gpu, &path).unwrap();
    opened.dry(&gpu);
    let volume = opened.canvas.read_f32(&gpu, StateTexture::WetVol);
    assert!(volume.iter().all(|&v| v == 0.0));
}

#[test]
fn opening_garbage_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let path = scratch("garbage.oleo");
    std::fs::write(&path, b"not a zip").unwrap();
    assert!(matches!(
        document::open(&gpu, &path),
        Err(DocumentError::Format(_))
    ));
}

#[test]
fn opening_a_missing_file_is_an_io_error() {
    let gpu = GpuContext::new().unwrap();
    let path = scratch("does-not-exist.oleo");
    let _ = std::fs::remove_file(&path);
    assert!(matches!(
        document::open(&gpu, &path),
        Err(DocumentError::Io(_))
    ));
}

#[test]
fn future_format_version_is_unsupported() {
    let gpu = GpuContext::new().unwrap();
    let s = painted_session(&gpu);
    let saved = scratch("future-source.oleo");
    let path = scratch("future.oleo");
    document::save(&gpu, &s, &saved).unwrap();
    rewrite_manifest(&saved, &path, |m| {
        m["format"] = 2.into();
        m["spec"] = serde_json::json!({ "something": "new" });
    });
    assert!(matches!(
        document::open(&gpu, &path),
        Err(DocumentError::UnsupportedVersion(2))
    ));
}

#[test]
fn unknown_pigment_in_manifest_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let s = painted_session(&gpu);
    let saved = scratch("pigment-source.oleo");
    let path = scratch("pigment.oleo");
    document::save(&gpu, &s, &saved).unwrap();
    rewrite_manifest(&saved, &path, |m| {
        m["spec"]["pigment_slots"][0] = "unobtainium".into();
    });
    assert!(matches!(
        document::open(&gpu, &path),
        Err(DocumentError::Format(_))
    ));
}

#[test]
fn oversized_canvas_in_manifest_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let s = painted_session(&gpu);
    let saved = scratch("size-source.oleo");
    let path = scratch("size.oleo");
    document::save(&gpu, &s, &saved).unwrap();
    rewrite_manifest(&saved, &path, |m| {
        m["spec"]["width"] = (MAX_CANVAS_SIZE + 1).into();
    });
    assert!(matches!(
        document::open(&gpu, &path),
        Err(DocumentError::Format(_))
    ));
}

#[test]
fn state_entry_with_wrong_length_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let s = painted_session(&gpu);
    let saved = scratch("length-source.oleo");
    let path = scratch("length.oleo");
    document::save(&gpu, &s, &saved).unwrap();
    rewrite_entries(&saved, &path, |name, bytes| {
        if name != "state/wet_vol.zst" {
            return Some(bytes);
        }
        let mut raw = zstd::decode_all(bytes.as_slice()).unwrap();
        raw.truncate(raw.len() - 4);
        Some(zstd::encode_all(raw.as_slice(), 3).unwrap())
    });
    assert!(matches!(
        document::open(&gpu, &path),
        Err(DocumentError::Format(_))
    ));
}

#[test]
fn missing_state_entry_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let s = painted_session(&gpu);
    let saved = scratch("missing-source.oleo");
    let path = scratch("missing.oleo");
    document::save(&gpu, &s, &saved).unwrap();
    rewrite_entries(&saved, &path, |name, bytes| {
        (name != "state/dry_height.zst").then_some(bytes)
    });
    assert!(matches!(
        document::open(&gpu, &path),
        Err(DocumentError::Format(_))
    ));
}

#[test]
fn saved_entries_are_stored_uncompressed_in_the_zip() {
    let gpu = GpuContext::new().unwrap();
    let s = painted_session(&gpu);
    let path = scratch("stored.oleo");
    document::save(&gpu, &s, &path).unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).unwrap();
        assert_eq!(entry.compression(), zip::CompressionMethod::Stored);
        names.push(entry.name().to_string());
    }
    for expected in [
        "manifest.json",
        "state/wet_pig0.zst",
        "state/wet_pig1.zst",
        "state/wet_vol.zst",
        "state/dry_refl0.zst",
        "state/dry_refl1.zst",
        "state/dry_height.zst",
        "state/dry_top_pig0.zst",
        "state/dry_top_pig1.zst",
    ] {
        assert!(names.iter().any(|n| n == expected), "{expected} missing");
    }
    let mut manifest = String::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    assert_eq!(manifest["format"], 1);
    assert_eq!(manifest["bands"], 8);
    assert_eq!(manifest["units"], "um");
}

#[test]
fn export_png_has_canvas_dimensions() {
    let gpu = GpuContext::new().unwrap();
    let mut s = PaintSession::new(
        &gpu,
        CanvasSpec {
            width: 300,
            height: 200,
            primer: Primer::WhiteGesso,
            weave: Weave::Smooth,
            pigment_slots: vec!["titanium_white".into()],
        },
    );
    let path = scratch("export.png");
    document::export_png(&gpu, &mut s, &path).unwrap();
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    assert_eq!((reader.info().width, reader.info().height), (300, 200));
    assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
    let mut pixels = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut pixels).unwrap();
    assert!(pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
}

#[test]
fn export_png_restores_the_view_mode_and_reshades_it() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session(&gpu);
    s.view_mode = ViewMode::Height;
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.canvas.dirty.mark_all();
    s.encode_frame(&gpu, &mut e);
    gpu.queue.submit([e.finish()]);
    let height_display = s.canvas.read_bytes(&gpu, StateTexture::Display);
    let path = scratch("export-mode.png");
    document::export_png(&gpu, &mut s, &path).unwrap();
    assert_eq!(s.view_mode, ViewMode::Height);
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(&gpu, &mut e);
    gpu.queue.submit([e.finish()]);
    assert_eq!(
        s.canvas.read_bytes(&gpu, StateTexture::Display),
        height_display
    );
}
