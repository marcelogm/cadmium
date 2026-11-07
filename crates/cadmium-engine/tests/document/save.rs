use std::io::Read;

use cadmium_engine::{
    canvas::{CanvasSpec, DirtyTiles, Primer, StateTexture, TILE_SIZE, TileCoord, Weave},
    document::{self, DocumentSnapshot},
    gpu::GpuContext,
    pigments::PigmentDb,
    session::PaintSession,
};

use crate::entries::{Entries, read_entries};
use crate::{
    BLANK_HEIGHT, BLANK_WIDTH, CURRENT_FORMAT, DRY_RELIEF, FORMAT_1_DRY_HEIGHT,
    FORMAT_1_STATE_ENTRIES, SKETCH_DENSITY, TempDir,
    painted_session, painted_session_with_stroke, painted_spec, sample, session_with_ended_stroke,
};
use crate::support::session;

const LARGE_WIDTH: u32 = 2048;
const LARGE_HEIGHT: u32 = 1536;
const MAX_ONE_STROKE_PAINTING_BYTES: u64 = 1 << 20;

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
    s.begin_stroke(sample(10.0, 30.0, 0));
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

#[test]
fn saving_keeps_the_graphite_sketch() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let mut sketch = vec![0.0; (s.canvas.spec.width * s.canvas.spec.height) as usize];
    sketch[0] = SKETCH_DENSITY;
    s.canvas.write_f32(&gpu, StateTexture::Graphite, &sketch);
    let dir = TempDir::new("graphite");
    let path = dir.file("graphite.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    let reopened = opened.canvas.read_f32(&gpu, StateTexture::Graphite).unwrap();
    assert_eq!(reopened, sketch);
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
    s.begin_stroke(sample(300.0, 700.0, 0));
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
    assert_eq!(manifest["format"], CURRENT_FORMAT);
    assert_eq!(manifest["bands"], 8);
    assert_eq!(manifest["units"], "um");
    drop(archive);
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
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
