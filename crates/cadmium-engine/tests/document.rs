use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use cadmium_engine::canvas::*;
use cadmium_engine::document::{self, DocumentError, DocumentSnapshot, ExportedImage};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::pigments::PigmentDb;
use cadmium_engine::render::ViewMode;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::{Surface, tray_spec};
use cadmium_engine::tube::Nozzle;

const BLANK_WIDTH: u32 = 64;
const BLANK_HEIGHT: u32 = 32;
const MAX_MANIFEST_BYTES: usize = 1 << 20;
const LARGE_WIDTH: u32 = 2048;
const LARGE_HEIGHT: u32 = 1536;
const MAX_ONE_STROKE_PAINTING_BYTES: u64 = 1 << 20;
const FORMAT_1_DRY_HEIGHT: &str = "state/dry_height.zst";
const DRY_RELIEF: &str = "state/dry_relief.zst";
const TRAY_PREFIX: &str = "tray/";
const TRAY_WET_VOLUME: &str = "tray/state/wet_vol.zst";
const TRAY_TOLERANCE: f64 = 1e-3;
const INVALID_THICKNESS: &str = "holds a negative or non-finite thickness";
const FORMAT_1_STATE_ENTRIES: [(&str, StateTexture); 8] = [
    ("state/wet_pig0.zst", StateTexture::WetPig0),
    ("state/wet_pig1.zst", StateTexture::WetPig1),
    ("state/wet_vol.zst", StateTexture::WetVol),
    ("state/dry_refl0.zst", StateTexture::DryRefl0),
    ("state/dry_refl1.zst", StateTexture::DryRefl1),
    (FORMAT_1_DRY_HEIGHT, StateTexture::DryHeight),
    ("state/dry_top_pig0.zst", StateTexture::DryTopPig0),
    ("state/dry_top_pig1.zst", StateTexture::DryTopPig1),
];

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let unique = format!(
            "cadmium-doc-{}-{}-{name}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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

fn painted_spec() -> CanvasSpec {
    CanvasSpec {
        width: 256,
        height: 128,
        primer: Primer::SiennaImprimatura,
        weave: Weave::Cotton,
        pigment_slots: vec!["burnt_sienna".into(), "titanium_white".into()],
    }
}

fn session_with_ended_stroke(gpu: &GpuContext, from: (f64, f64), to: (f64, f64)) -> PaintSession {
    let mut s = PaintSession::new(gpu, painted_spec()).unwrap();
    s.begin_stroke(gpu, sample(from.0, from.1, 0));
    s.add_samples(&[sample(to.0, to.1, 200)]);
    s.end_stroke();
    s
}

fn painted_session(gpu: &GpuContext) -> PaintSession {
    painted_session_with_stroke(gpu, (20.0, 60.0), (230.0, 70.0))
}

fn painted_session_with_stroke(gpu: &GpuContext, from: (f64, f64), to: (f64, f64)) -> PaintSession {
    let mut s = session_with_ended_stroke(gpu, from, to);
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(gpu, &mut e);
    gpu.queue.submit([e.finish()]);
    s
}

struct Entries(Vec<(String, Vec<u8>)>);

impl Entries {
    fn blank() -> Entries {
        let spec = CanvasSpec {
            width: BLANK_WIDTH,
            height: BLANK_HEIGHT,
            primer: Primer::WhiteGesso,
            weave: Weave::Smooth,
            pigment_slots: vec!["titanium_white".into()],
        };
        let manifest = serde_json::json!({
            "format": 1,
            "pigment_db": PigmentDb::VERSION,
            "spec": spec,
            "bands": 8,
            "units": "um",
        });
        let mut entries = vec![(
            "manifest.json".to_string(),
            serde_json::to_vec(&manifest).unwrap(),
        )];
        for (name, which) in FORMAT_1_STATE_ENTRIES {
            let raw = vec![0u8; (BLANK_WIDTH * BLANK_HEIGHT * which.bytes_per_texel()) as usize];
            entries.push((
                name.to_string(),
                zstd::encode_all(raw.as_slice(), 3).unwrap(),
            ));
        }
        Entries(entries)
    }

    fn bytes(&mut self, name: &str) -> &mut Vec<u8> {
        &mut self
            .0
            .iter_mut()
            .find(|(entry, _)| entry == name)
            .unwrap()
            .1
    }

    fn edit_manifest(mut self, edit: impl FnOnce(&mut serde_json::Value)) -> Entries {
        let bytes = self.bytes("manifest.json");
        let mut manifest: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        edit(&mut manifest);
        *bytes = serde_json::to_vec(&manifest).unwrap();
        self
    }

    fn edit_state(mut self, name: &str, edit: impl FnOnce(&mut Vec<u8>)) -> Entries {
        let bytes = self.bytes(name);
        let mut raw = zstd::decode_all(bytes.as_slice()).unwrap();
        edit(&mut raw);
        *bytes = zstd::encode_all(raw.as_slice(), 3).unwrap();
        self
    }

    fn in_format_2(mut self) -> Entries {
        for (entry, _) in &mut self.0 {
            if entry == FORMAT_1_DRY_HEIGHT {
                *entry = DRY_RELIEF.to_string();
            }
        }
        self.edit_manifest(|m| m["format"] = 2.into())
    }

    fn saved(path: &Path) -> Entries {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let entries = (0..archive.len())
            .map(|index| {
                let mut entry = archive.by_index(index).unwrap();
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                (entry.name().to_string(), bytes)
            })
            .collect();
        Entries(entries)
    }

    fn without_tray(mut self) -> Entries {
        self.0.retain(|(entry, _)| !entry.starts_with(TRAY_PREFIX));
        self
    }

    fn without(mut self, name: &str) -> Entries {
        self.0.retain(|(entry, _)| entry != name);
        self
    }

    fn write(&self, path: &Path) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in &self.0 {
            writer.start_file(name.as_str(), options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
}

fn read_entries(entries: Entries) -> Result<DocumentSnapshot, DocumentError> {
    let dir = TempDir::new("read");
    let path = dir.file("painting.cadmium");
    entries.write(&path);
    DocumentSnapshot::read(&path)
}

fn format_reason(result: Result<DocumentSnapshot, DocumentError>) -> String {
    match result {
        Err(DocumentError::Format(reason)) => reason,
        Err(other) => panic!("expected a format error, got {other:?}"),
        Ok(_) => panic!("expected a format error, the document was accepted"),
    }
}

fn set_first_thickness(value: f32) -> impl FnOnce(&mut Vec<u8>) {
    move |raw| raw[..4].copy_from_slice(&value.to_le_bytes())
}

#[test]
fn reading_a_well_formed_document_needs_no_gpu() {
    assert!(read_entries(Entries::blank()).is_ok());
}

#[test]
fn reading_garbage_is_a_format_error() {
    let dir = TempDir::new("garbage");
    let path = dir.file("garbage.cadmium");
    std::fs::write(&path, b"not a zip").unwrap();
    assert!(matches!(
        DocumentSnapshot::read(&path),
        Err(DocumentError::Format(_))
    ));
}

#[test]
fn reading_a_missing_file_is_an_io_error() {
    let dir = TempDir::new("missing-file");
    assert!(matches!(
        DocumentSnapshot::read(&dir.file("does-not-exist.cadmium")),
        Err(DocumentError::Io(_))
    ));
}

#[test]
fn future_format_version_is_unsupported() {
    let entries = Entries::blank().edit_manifest(|m| {
        m["format"] = 4.into();
        m["spec"] = serde_json::json!({ "something": "new" });
    });
    assert!(matches!(
        read_entries(entries),
        Err(DocumentError::UnsupportedVersion(4))
    ));
}

#[test]
fn unknown_pigment_in_manifest_is_a_format_error() {
    let entries = Entries::blank().edit_manifest(|m| {
        m["spec"]["pigment_slots"][0] = "unobtainium".into();
    });
    let reason = format_reason(read_entries(entries));
    assert!(
        reason.contains("unknown pigment \"unobtainium\""),
        "{reason}"
    );
}

#[test]
fn oversized_canvas_in_manifest_is_a_format_error() {
    let entries = Entries::blank().edit_manifest(|m| {
        m["spec"]["width"] = (MAX_CANVAS_SIZE + 1).into();
    });
    let reason = format_reason(read_entries(entries));
    assert!(reason.contains("canvas size"), "{reason}");
    assert!(reason.contains(&MAX_CANVAS_SIZE.to_string()), "{reason}");
}

#[test]
fn oversized_manifest_is_a_format_error() {
    let entries = Entries::blank().edit_manifest(|m| {
        m["padding"] = "x".repeat(MAX_MANIFEST_BYTES).into();
    });
    let reason = format_reason(read_entries(entries));
    assert!(reason.contains("too large"), "{reason}");
}

#[test]
fn missing_manifest_is_a_format_error() {
    let reason = format_reason(read_entries(Entries::blank().without("manifest.json")));
    assert!(reason.contains("missing manifest.json"), "{reason}");
}

#[test]
fn missing_state_entry_is_a_format_error() {
    let reason = format_reason(read_entries(
        Entries::blank().without("state/dry_height.zst"),
    ));
    assert!(reason.contains("missing state/dry_height.zst"), "{reason}");
}

#[test]
fn state_entry_shorter_than_the_canvas_is_a_format_error() {
    let entries = Entries::blank().edit_state("state/wet_vol.zst", |raw| {
        raw.truncate(raw.len() - 4);
    });
    let reason = format_reason(read_entries(entries));
    assert!(reason.contains("state/wet_vol.zst holds"), "{reason}");
}

#[test]
fn state_entry_decompressing_past_the_canvas_is_a_format_error() {
    let entries = Entries::blank().edit_state("state/wet_pig0.zst", |raw| {
        raw.extend(std::iter::repeat_n(0, 1 << 16));
    });
    let reason = format_reason(read_entries(entries));
    assert!(reason.contains("state/wet_pig0.zst holds"), "{reason}");
}

#[test]
fn non_finite_or_negative_wet_volume_is_a_format_error() {
    for value in [f32::NAN, f32::INFINITY, -1.0] {
        let entries = Entries::blank().edit_state("state/wet_vol.zst", set_first_thickness(value));
        let reason = format_reason(read_entries(entries));
        assert!(
            reason.contains(&format!("state/wet_vol.zst {INVALID_THICKNESS}")),
            "{reason}"
        );
    }
}

#[test]
fn non_finite_or_negative_dry_height_is_a_format_error() {
    for value in [f32::NAN, f32::NEG_INFINITY, -0.5] {
        let entries = Entries::blank().edit_state(FORMAT_1_DRY_HEIGHT, set_first_thickness(value));
        let reason = format_reason(read_entries(entries));
        assert!(
            reason.contains(&format!("{FORMAT_1_DRY_HEIGHT} {INVALID_THICKNESS}")),
            "{reason}"
        );
    }
}

#[test]
fn a_dry_relief_that_sinks_below_the_bare_canvas_is_a_format_error() {
    for value in [f32::NAN, f32::NEG_INFINITY, -0.5] {
        let entries = Entries::blank()
            .in_format_2()
            .edit_state(DRY_RELIEF, set_first_thickness(value));
        let reason = format_reason(read_entries(entries));
        assert!(
            reason.contains(&format!("{DRY_RELIEF} {INVALID_THICKNESS}")),
            "{reason}"
        );
    }
}

#[test]
fn a_well_formed_format_2_document_reads() {
    assert!(read_entries(Entries::blank().in_format_2()).is_ok());
}

#[test]
fn save_then_open_restores_every_state_texture() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session(&gpu);
    let dir = TempDir::new("round-trip");
    let path = dir.file("t.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    assert_eq!(opened.canvas.spec, s.canvas.spec);
    for t in StateTexture::PAINT_STATE {
        assert_eq!(
            opened.canvas.read_bytes(&gpu, t).unwrap(),
            s.canvas.read_bytes(&gpu, t).unwrap(),
            "{t:?}"
        );
    }
}

#[test]
fn a_canvas_whose_rows_need_padding_survives_save_and_open() {
    const UNALIGNED_WIDTH: u32 = 100;
    const UNALIGNED_HEIGHT: u32 = 60;
    let gpu = GpuContext::new().unwrap();
    let spec = CanvasSpec {
        width: UNALIGNED_WIDTH,
        height: UNALIGNED_HEIGHT,
        ..painted_spec()
    };
    let mut s = PaintSession::new(&gpu, spec).unwrap();
    s.begin_stroke(&gpu, sample(10.0, 30.0, 0));
    s.add_samples(&[sample(90.0, 32.0, 200)]);
    s.end_stroke();
    let dir = TempDir::new("unaligned");
    let path = dir.file("t.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    assert!(s.canvas.wet_totals(&gpu).unwrap().volume > 0.0);
    for t in StateTexture::PAINT_STATE {
        assert_eq!(
            opened.canvas.read_bytes(&gpu, t).unwrap(),
            s.canvas.read_bytes(&gpu, t).unwrap(),
            "{t:?}"
        );
    }
}

fn dried_session(gpu: &GpuContext) -> PaintSession {
    let mut s = painted_session(gpu);
    s.dry(gpu);
    s
}

#[test]
fn a_dried_stroke_survives_save_and_open_bit_for_bit() {
    let gpu = GpuContext::new().unwrap();
    let mut s = dried_session(&gpu);
    let dir = TempDir::new("dried");
    let path = dir.file("dried.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    for t in StateTexture::PAINT_STATE {
        assert_eq!(
            opened.canvas.read_bytes(&gpu, t).unwrap(),
            s.canvas.read_bytes(&gpu, t).unwrap(),
            "{t:?}"
        );
    }
}

#[test]
fn a_format_1_painting_opens_with_the_dry_height_it_stored() {
    let gpu = GpuContext::new().unwrap();
    let s = dried_session(&gpu);
    let manifest = serde_json::json!({
        "format": 1,
        "pigment_db": PigmentDb::VERSION,
        "spec": s.canvas.spec,
        "bands": 8,
        "units": "um",
    });
    let mut entries = vec![(
        "manifest.json".to_string(),
        serde_json::to_vec(&manifest).unwrap(),
    )];
    for (name, which) in FORMAT_1_STATE_ENTRIES {
        let raw = s.canvas.read_bytes(&gpu, which).unwrap();
        entries.push((
            name.to_string(),
            zstd::encode_all(raw.as_slice(), 3).unwrap(),
        ));
    }
    let opened = read_entries(Entries(entries))
        .unwrap()
        .into_session(&gpu)
        .unwrap();
    for t in StateTexture::PAINT_STATE {
        assert_eq!(
            opened.canvas.read_bytes(&gpu, t).unwrap(),
            s.canvas.read_bytes(&gpu, t).unwrap(),
            "{t:?}"
        );
    }
}

#[test]
fn a_large_painting_with_one_stroke_saves_without_its_weave() {
    let gpu = GpuContext::new().unwrap();
    let mut s = PaintSession::new(
        &gpu,
        CanvasSpec {
            width: LARGE_WIDTH,
            height: LARGE_HEIGHT,
            primer: Primer::WhiteGesso,
            weave: Weave::Linen,
            pigment_slots: vec!["ultramarine_blue".into()],
        },
    )
    .unwrap();
    s.begin_stroke(&gpu, sample(300.0, 700.0, 0));
    s.add_samples(&[sample(1700.0, 800.0, 400)]);
    s.end_stroke();
    let dir = TempDir::new("large");
    let path = dir.file("large.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let size = std::fs::metadata(&path).unwrap().len();
    eprintln!("a {LARGE_WIDTH}×{LARGE_HEIGHT} painting with one stroke saves in {size} bytes");
    assert!(size <= MAX_ONE_STROKE_PAINTING_BYTES, "{size}");
    let opened = document::open(&gpu, &path).unwrap();
    assert_eq!(
        opened
            .canvas
            .read_bytes(&gpu, StateTexture::DryHeight)
            .unwrap(),
        s.canvas.read_bytes(&gpu, StateTexture::DryHeight).unwrap()
    );
}

#[test]
fn saving_right_after_a_stroke_ends_keeps_the_stroke() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session_with_ended_stroke(&gpu, (20.0, 60.0), (230.0, 70.0));
    let dir = TempDir::new("unflushed");
    let path = dir.file("t.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    let volume = opened.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
    assert!(volume.iter().any(|&v| v > 0.0));
    assert!(s.history.can_undo());
}

#[test]
fn opened_document_marks_every_tile_dirty_and_only_painted_tiles_wet() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session_with_stroke(&gpu, (20.0, 20.0), (40.0, 30.0));
    let dir = TempDir::new("wet");
    let path = dir.file("wet.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    let mut full = DirtyTiles::new(256, 128);
    full.mark_all();
    assert_eq!(opened.canvas.dirty.tiles(), full.tiles());
    let wet = opened.canvas.wet_tiles.tiles();
    assert!(!wet.is_empty());
    assert!(wet.len() < full.tiles().len());
    let volume = opened.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
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
fn a_snapshot_read_off_the_main_thread_becomes_a_session() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session(&gpu);
    let dir = TempDir::new("worker");
    let path = dir.file("t.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let snapshot = std::thread::spawn(move || DocumentSnapshot::read(&path))
        .join()
        .unwrap()
        .unwrap();
    let opened = snapshot.into_session(&gpu).unwrap();
    assert_eq!(
        opened
            .canvas
            .read_bytes(&gpu, StateTexture::WetVol)
            .unwrap(),
        s.canvas.read_bytes(&gpu, StateTexture::WetVol).unwrap()
    );
}

#[test]
fn drying_an_opened_document_clears_its_wet_layer() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session(&gpu);
    let dir = TempDir::new("dry");
    let path = dir.file("dry.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let mut opened = document::open(&gpu, &path).unwrap();
    opened.dry(&gpu);
    let volume = opened.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
    assert!(volume.iter().all(|&v| v == 0.0));
}

#[test]
fn saved_entries_are_stored_uncompressed_in_the_zip() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session(&gpu);
    let dir = TempDir::new("stored");
    let path = dir.file("stored.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).unwrap();
        assert_eq!(entry.compression(), zip::CompressionMethod::Stored);
        names.push(entry.name().to_string());
    }
    let saved_entries = FORMAT_1_STATE_ENTRIES.map(|(name, _)| {
        if name == FORMAT_1_DRY_HEIGHT {
            DRY_RELIEF
        } else {
            name
        }
    });
    for expected in std::iter::once("manifest.json").chain(saved_entries) {
        assert!(names.iter().any(|n| n == expected), "{expected} missing");
    }
    let mut manifest = String::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    assert_eq!(manifest["format"], 3);
    assert_eq!(manifest["bands"], 8);
    assert_eq!(manifest["units"], "um");
    drop(archive);
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
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
    )
    .unwrap();
    let dir = TempDir::new("export");
    let path = dir.file("export.png");
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
    let height_display = s.canvas.read_bytes(&gpu, StateTexture::Display).unwrap();
    let dir = TempDir::new("export-mode");
    let path = dir.file("export-mode.png");
    document::export_png(&gpu, &mut s, &path).unwrap();
    assert_eq!(s.view_mode, ViewMode::Height);
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(&gpu, &mut e);
    gpu.queue.submit([e.finish()]);
    assert_eq!(
        s.canvas.read_bytes(&gpu, StateTexture::Display).unwrap(),
        height_display
    );
}

#[test]
fn a_snapshot_restored_into_a_running_session_replaces_its_painting() {
    let gpu = GpuContext::new().unwrap();
    let mut saved = painted_session(&gpu);
    let snapshot = DocumentSnapshot::capture(&gpu, &mut saved).unwrap();
    let mut running = PaintSession::new(
        &gpu,
        CanvasSpec {
            width: BLANK_WIDTH,
            height: BLANK_HEIGHT,
            ..painted_spec()
        },
    )
    .unwrap();
    snapshot.restore_into(&gpu, &mut running).unwrap();
    assert_eq!(running.canvas.spec, saved.canvas.spec);
    assert!(!running.canvas.wet_tiles.is_empty());
    for t in StateTexture::PAINT_STATE {
        assert_eq!(
            running.canvas.read_bytes(&gpu, t).unwrap(),
            saved.canvas.read_bytes(&gpu, t).unwrap(),
            "{t:?}"
        );
    }
}

fn squeeze_on_the_tray(gpu: &GpuContext, s: &mut PaintSession) {
    s.begin_hold(gpu, Surface::Tray);
    s.squeeze(
        gpu,
        Nozzle {
            from: [300.0, 300.0],
            to: [300.0, 300.0],
            radius: 12.0,
            slot: 1,
            pressure: 1.0,
            seconds: 0.2,
        },
    );
    s.end_hold();
}

fn painted_session_with_a_wet_tray(gpu: &GpuContext) -> PaintSession {
    let mut s = painted_session(gpu);
    squeeze_on_the_tray(gpu, &mut s);
    s
}

fn assert_same_tray_paint(gpu: &GpuContext, opened: &PaintSession, saved: &PaintSession) {
    let opened = opened.tray.wet_totals(gpu).unwrap();
    let saved = saved.tray.wet_totals(gpu).unwrap();
    assert!(saved.volume > 0.0);
    let close = |a: f64, b: f64| (a - b).abs() <= TRAY_TOLERANCE * b.abs();
    assert!(close(opened.volume, saved.volume), "{opened:?} {saved:?}");
    for (opened, saved) in opened.pigment.iter().zip(saved.pigment) {
        assert!(close(*opened, saved), "{opened} {saved}");
    }
}

fn saved_painting_with_a_wet_tray(gpu: &GpuContext, dir: &TempDir) -> (PaintSession, PathBuf) {
    let mut s = painted_session_with_a_wet_tray(gpu);
    let path = dir.file("tray.cadmium");
    document::save(gpu, &mut s, &path).unwrap();
    (s, path)
}

#[test]
fn the_tray_is_saved_and_opened_with_the_painting() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("tray");
    let (s, path) = saved_painting_with_a_wet_tray(&gpu, &dir);
    let opened = document::open(&gpu, &path).unwrap();
    assert_eq!(opened.tray.spec, tray_spec(&s.canvas.spec.pigment_slots));
    assert_same_tray_paint(&gpu, &opened, &s);
    assert!(!opened.tray.wet_tiles.is_empty());
    assert_eq!(
        opened
            .canvas
            .read_bytes(&gpu, StateTexture::WetVol)
            .unwrap(),
        s.canvas.read_bytes(&gpu, StateTexture::WetVol).unwrap()
    );
}

#[test]
fn a_snapshot_restored_into_a_running_session_replaces_its_tray() {
    let gpu = GpuContext::new().unwrap();
    let mut saved = painted_session_with_a_wet_tray(&gpu);
    let snapshot = DocumentSnapshot::capture(&gpu, &mut saved).unwrap();
    let mut running = PaintSession::new(&gpu, painted_spec()).unwrap();
    snapshot.restore_into(&gpu, &mut running).unwrap();
    assert_same_tray_paint(&gpu, &running, &saved);
}

#[test]
fn a_format_two_painting_opens_with_a_clean_tray() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("format-2-tray");
    let (s, path) = saved_painting_with_a_wet_tray(&gpu, &dir);
    let entries = Entries::saved(&path)
        .without_tray()
        .edit_manifest(|m| m["format"] = 2.into());
    let opened = read_entries(entries).unwrap().into_session(&gpu).unwrap();
    assert_eq!(opened.tray.wet_totals(&gpu).unwrap().volume, 0.0);
    assert_eq!(
        opened.tray.spec,
        tray_spec(&opened.canvas.spec.pigment_slots)
    );
    assert_eq!(
        opened
            .canvas
            .read_bytes(&gpu, StateTexture::WetVol)
            .unwrap(),
        s.canvas.read_bytes(&gpu, StateTexture::WetVol).unwrap()
    );
}

#[test]
fn a_format_two_painting_opened_into_a_running_session_clears_its_tray() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("format-2-running");
    let (mut running, path) = saved_painting_with_a_wet_tray(&gpu, &dir);
    let entries = Entries::saved(&path)
        .without_tray()
        .edit_manifest(|m| m["format"] = 2.into());
    read_entries(entries)
        .unwrap()
        .restore_into(&gpu, &mut running)
        .unwrap();
    assert_eq!(running.tray.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(running.tray.wet_tiles.is_empty());
}

#[test]
fn a_format_two_painting_saved_again_still_opens() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("format-2-resaved");
    let (_, path) = saved_painting_with_a_wet_tray(&gpu, &dir);
    let entries = Entries::saved(&path)
        .without_tray()
        .edit_manifest(|m| m["format"] = 2.into());
    let resaved = dir.file("resaved.cadmium");
    read_entries(entries).unwrap().write(&resaved).unwrap();
    let opened = document::open(&gpu, &resaved).unwrap();
    assert_eq!(opened.tray.wet_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn a_format_three_painting_missing_its_tray_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("format-3-trayless");
    let (_, path) = saved_painting_with_a_wet_tray(&gpu, &dir);
    let reason = format_reason(read_entries(Entries::saved(&path).without(TRAY_WET_VOLUME)));
    assert!(
        reason.contains(&format!("missing {TRAY_WET_VOLUME}")),
        "{reason}"
    );
}

#[test]
fn a_tray_entry_shorter_than_the_tray_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("short-tray");
    let (_, path) = saved_painting_with_a_wet_tray(&gpu, &dir);
    let entries = Entries::saved(&path).edit_state(TRAY_WET_VOLUME, |raw| {
        raw.truncate(raw.len() - 4);
    });
    let reason = format_reason(read_entries(entries));
    assert!(
        reason.contains(&format!("{TRAY_WET_VOLUME} holds")),
        "{reason}"
    );
}

#[test]
fn a_tray_entry_with_a_negative_thickness_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("negative-tray");
    let (_, path) = saved_painting_with_a_wet_tray(&gpu, &dir);
    let entries = Entries::saved(&path).edit_state(TRAY_WET_VOLUME, set_first_thickness(-1.0));
    let reason = format_reason(read_entries(entries));
    assert!(
        reason.contains(&format!("{TRAY_WET_VOLUME} {INVALID_THICKNESS}")),
        "{reason}"
    );
}

#[test]
fn export_ignores_the_tray() {
    let gpu = GpuContext::new().unwrap();
    let mut fresh = PaintSession::new(&gpu, painted_spec()).unwrap();
    let mut with_tray_paint = PaintSession::new(&gpu, painted_spec()).unwrap();
    squeeze_on_the_tray(&gpu, &mut with_tray_paint);
    assert!(with_tray_paint.tray.wet_totals(&gpu).unwrap().volume > 0.0);
    let fresh = ExportedImage::render(&gpu, &mut fresh).unwrap();
    let with_tray_paint = ExportedImage::render(&gpu, &mut with_tray_paint).unwrap();
    assert_eq!(
        (with_tray_paint.width, with_tray_paint.height),
        (fresh.width, fresh.height)
    );
    assert!(with_tray_paint.rgba == fresh.rgba);
}
