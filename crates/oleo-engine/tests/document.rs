use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use oleo_engine::canvas::*;
use oleo_engine::document::{self, DocumentError, DocumentSnapshot};
use oleo_engine::gpu::GpuContext;
use oleo_engine::pigments::PigmentDb;
use oleo_engine::render::ViewMode;
use oleo_engine::session::PaintSession;
use oleo_engine::stroke::StrokeSample;

const BLANK_WIDTH: u32 = 64;
const BLANK_HEIGHT: u32 = 32;
const MAX_MANIFEST_BYTES: usize = 1 << 20;
const STATE_ENTRIES: [(&str, StateTexture); 8] = [
    ("state/wet_pig0.zst", StateTexture::WetPig0),
    ("state/wet_pig1.zst", StateTexture::WetPig1),
    ("state/wet_vol.zst", StateTexture::WetVol),
    ("state/dry_refl0.zst", StateTexture::DryRefl0),
    ("state/dry_refl1.zst", StateTexture::DryRefl1),
    ("state/dry_height.zst", StateTexture::DryHeight),
    ("state/dry_top_pig0.zst", StateTexture::DryTopPig0),
    ("state/dry_top_pig1.zst", StateTexture::DryTopPig1),
];

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let unique = format!(
            "oleo-doc-{}-{}-{name}",
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
    let mut s = PaintSession::new(gpu, painted_spec());
    s.begin_stroke(gpu, sample(from.0, from.1, 0));
    s.add_samples(&[sample(to.0, to.1, 200)]);
    s.end_stroke(gpu);
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
        for (name, which) in STATE_ENTRIES {
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
    let path = dir.file("painting.oleo");
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
    let path = dir.file("garbage.oleo");
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
        DocumentSnapshot::read(&dir.file("does-not-exist.oleo")),
        Err(DocumentError::Io(_))
    ));
}

#[test]
fn future_format_version_is_unsupported() {
    let entries = Entries::blank().edit_manifest(|m| {
        m["format"] = 2.into();
        m["spec"] = serde_json::json!({ "something": "new" });
    });
    assert!(matches!(
        read_entries(entries),
        Err(DocumentError::UnsupportedVersion(2))
    ));
}

#[test]
fn unknown_pigment_in_manifest_is_a_format_error() {
    let entries = Entries::blank().edit_manifest(|m| {
        m["spec"]["pigment_slots"][0] = "unobtainium".into();
    });
    format_reason(read_entries(entries));
}

#[test]
fn oversized_canvas_in_manifest_is_a_format_error() {
    let entries = Entries::blank().edit_manifest(|m| {
        m["spec"]["width"] = (MAX_CANVAS_SIZE + 1).into();
    });
    format_reason(read_entries(entries));
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
    format_reason(read_entries(entries));
}

#[test]
fn state_entry_decompressing_past_the_canvas_is_a_format_error() {
    let entries = Entries::blank().edit_state("state/wet_pig0.zst", |raw| {
        raw.extend(std::iter::repeat_n(0, 1 << 16));
    });
    format_reason(read_entries(entries));
}

#[test]
fn non_finite_or_negative_wet_volume_is_a_format_error() {
    for value in [f32::NAN, f32::INFINITY, -1.0] {
        let entries = Entries::blank().edit_state("state/wet_vol.zst", set_first_thickness(value));
        format_reason(read_entries(entries));
    }
}

#[test]
fn non_finite_or_negative_dry_height_is_a_format_error() {
    for value in [f32::NAN, f32::NEG_INFINITY, -0.5] {
        let entries =
            Entries::blank().edit_state("state/dry_height.zst", set_first_thickness(value));
        format_reason(read_entries(entries));
    }
}

#[test]
fn save_then_open_restores_every_state_texture() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session(&gpu);
    let dir = TempDir::new("round-trip");
    let path = dir.file("t.oleo");
    document::save(&gpu, &mut s, &path).unwrap();
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
fn saving_right_after_a_stroke_ends_keeps_the_stroke() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session_with_ended_stroke(&gpu, (20.0, 60.0), (230.0, 70.0));
    let dir = TempDir::new("unflushed");
    let path = dir.file("t.oleo");
    document::save(&gpu, &mut s, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    let volume = opened.canvas.read_f32(&gpu, StateTexture::WetVol);
    assert!(volume.iter().any(|&v| v > 0.0));
    assert!(s.history.can_undo());
}

#[test]
fn opened_document_marks_every_tile_dirty_and_only_painted_tiles_wet() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session_with_stroke(&gpu, (20.0, 20.0), (40.0, 30.0));
    let dir = TempDir::new("wet");
    let path = dir.file("wet.oleo");
    document::save(&gpu, &mut s, &path).unwrap();
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
fn a_snapshot_read_off_the_main_thread_becomes_a_session() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session(&gpu);
    let dir = TempDir::new("worker");
    let path = dir.file("t.oleo");
    document::save(&gpu, &mut s, &path).unwrap();
    let snapshot = std::thread::spawn(move || DocumentSnapshot::read(&path))
        .join()
        .unwrap()
        .unwrap();
    let opened = snapshot.into_session(&gpu);
    assert_eq!(
        opened.canvas.read_bytes(&gpu, StateTexture::WetVol),
        s.canvas.read_bytes(&gpu, StateTexture::WetVol)
    );
}

#[test]
fn drying_an_opened_document_clears_its_wet_layer() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session(&gpu);
    let dir = TempDir::new("dry");
    let path = dir.file("dry.oleo");
    document::save(&gpu, &mut s, &path).unwrap();
    let mut opened = document::open(&gpu, &path).unwrap();
    opened.dry(&gpu);
    let volume = opened.canvas.read_f32(&gpu, StateTexture::WetVol);
    assert!(volume.iter().all(|&v| v == 0.0));
}

#[test]
fn saved_entries_are_stored_uncompressed_in_the_zip() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session(&gpu);
    let dir = TempDir::new("stored");
    let path = dir.file("stored.oleo");
    document::save(&gpu, &mut s, &path).unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).unwrap();
        assert_eq!(entry.compression(), zip::CompressionMethod::Stored);
        names.push(entry.name().to_string());
    }
    for expected in std::iter::once("manifest.json").chain(STATE_ENTRIES.map(|(name, _)| name)) {
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
    );
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
    let height_display = s.canvas.read_bytes(&gpu, StateTexture::Display);
    let dir = TempDir::new("export-mode");
    let path = dir.file("export-mode.png");
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
