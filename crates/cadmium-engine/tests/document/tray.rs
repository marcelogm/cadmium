use std::path::{Path, PathBuf};

use cadmium_engine::{
    canvas::{Canvas, CanvasSpec, StateTexture, TILE_SIZE, TileCoord},
    document::{self, DocumentSnapshot},
    gpu::GpuContext,
    session::PaintSession,
    surface::{TRAY_HEIGHT, TRAY_WIDTH, tray_spec},
};

use crate::entries::{Entries, format_reason, read_entries, set_first_f32};
use crate::{
    CURRENT_FORMAT, INVALID_THICKNESS, TempDir, painted_session, painted_spec,
    sketch_on_the_painting, squeeze_on_the_tray, squeeze_on_the_tray_at,
};

const TRAY_WET_VOLUME: &str = "tray/state/wet_vol.zst";
const TRAY_TOLERANCE: f64 = 1e-3;

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

fn middle_of_the_tray() -> [f32; 2] {
    [TRAY_WIDTH as f32 / 2.0, TRAY_HEIGHT as f32 / 2.0]
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
fn a_format_three_tray_opens_in_the_middle_of_the_larger_tray() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("format-3-tray");
    let mut s = painted_session(&gpu);
    squeeze_on_the_tray_at(&gpu, &mut s, middle_of_the_tray());
    let path = dir.file("tray.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let opened = read_entries(Entries::saved_as_format_3(&path))
        .unwrap()
        .into_session(&gpu)
        .unwrap();
    assert_eq!(
        opened.tray.spec,
        tray_spec(&opened.canvas.spec.pigment_slots)
    );
    assert_same_tray_paint(&gpu, &opened, &s);
    for t in StateTexture::PAINT_STATE {
        assert_eq!(
            opened.tray.read_bytes(&gpu, t).unwrap(),
            s.tray.read_bytes(&gpu, t).unwrap(),
            "the old tray's {t:?} lands where it was cut from, the margins stay clean"
        );
    }
    let under_the_squeeze = TileCoord {
        x: TRAY_WIDTH / 2 / TILE_SIZE,
        y: TRAY_HEIGHT / 2 / TILE_SIZE,
    };
    assert!(opened.tray.wet_tiles.contains(under_the_squeeze));
    assert!(!opened.tray.wet_tiles.contains(TileCoord { x: 0, y: 0 }));
}

#[test]
fn a_format_three_painting_saved_again_keeps_its_tray() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("format-3-resaved");
    let mut s = painted_session(&gpu);
    squeeze_on_the_tray_at(&gpu, &mut s, middle_of_the_tray());
    let path = dir.file("tray.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let resaved = dir.file("resaved.cadmium");
    read_entries(Entries::saved_as_format_3(&path))
        .unwrap()
        .write(&resaved)
        .unwrap();
    assert_eq!(saved_format(&resaved), 3);
    let opened = document::open(&gpu, &resaved).unwrap();
    assert_same_tray_paint(&gpu, &opened, &s);
}

#[test]
fn a_painting_with_a_wet_tray_saves_in_the_current_format() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("current-format");
    let (_, path) = saved_painting_with_a_wet_tray(&gpu, &dir);
    assert_eq!(saved_format(&path), CURRENT_FORMAT);
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
    let entries = Entries::saved_as_format_2(&path);
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
    let entries = Entries::saved_as_format_2(&path);
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
    let entries = Entries::saved_as_format_2(&path);
    let resaved = dir.file("resaved.cadmium");
    read_entries(entries).unwrap().write(&resaved).unwrap();
    let opened = document::open(&gpu, &resaved).unwrap();
    assert_eq!(opened.tray.wet_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn a_legacy_tray_painting_with_graphite_saves_in_the_current_format() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("format-3-graphite");
    let mut s = painted_session(&gpu);
    squeeze_on_the_tray_at(&gpu, &mut s, middle_of_the_tray());
    sketch_on_the_painting(&gpu, &s);
    let path = dir.file("tray.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let resaved = dir.file("resaved.cadmium");
    read_entries(Entries::saved_as_format_3(&path))
        .unwrap()
        .write(&resaved)
        .unwrap();
    assert_eq!(saved_format(&resaved), CURRENT_FORMAT);
}

#[test]
fn a_trayless_painting_with_graphite_saves_in_the_current_format() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("format-2-graphite");
    let (mut s, path) = saved_painting_with_a_wet_tray(&gpu, &dir);
    sketch_on_the_painting(&gpu, &s);
    document::save(&gpu, &mut s, &path).unwrap();
    let resaved = dir.file("resaved.cadmium");
    read_entries(Entries::saved_as_format_2(&path))
        .unwrap()
        .write(&resaved)
        .unwrap();
    assert_eq!(saved_format(&resaved), CURRENT_FORMAT);
}

fn saved_format(path: &Path) -> serde_json::Value {
    Entries::saved(path).manifest()["format"].clone()
}

#[test]
fn a_clean_tray_is_still_saved_in_the_current_format() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session(&gpu);
    let dir = TempDir::new("clean-tray");
    let path = dir.file("clean.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    assert_eq!(saved_format(&path), CURRENT_FORMAT);
    assert!(
        Entries::saved(&path)
            .0
            .iter()
            .any(|(entry, _)| entry == TRAY_WET_VOLUME)
    );
}

#[test]
fn a_painting_whose_tray_could_not_be_rebuilt_saves_without_it_and_reopens() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painted_session_with_a_wet_tray(&gpu);
    let placeholder = CanvasSpec {
        width: 1,
        height: 1,
        ..tray_spec(&s.canvas.spec.pigment_slots)
    };
    s.tray = Canvas::new(&gpu, &s.db, placeholder).unwrap();
    let dir = TempDir::new("placeholder-tray");
    let path = dir.file("placeholder.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    assert_eq!(saved_format(&path), 2);
    let opened = document::open(&gpu, &path).unwrap();
    assert_eq!(opened.tray.wet_totals(&gpu).unwrap().volume, 0.0);
    assert_eq!(
        opened.tray.spec,
        tray_spec(&opened.canvas.spec.pigment_slots)
    );
    for t in StateTexture::PAINT_STATE {
        assert_eq!(
            opened.canvas.read_bytes(&gpu, t).unwrap(),
            s.canvas.read_bytes(&gpu, t).unwrap(),
            "{t:?}"
        );
    }
}

#[test]
fn a_painting_missing_its_tray_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("trayless");
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
    let entries = Entries::saved(&path).edit_state(TRAY_WET_VOLUME, set_first_f32(-1.0));
    let reason = format_reason(read_entries(entries));
    assert!(
        reason.contains(&format!("{TRAY_WET_VOLUME} {INVALID_THICKNESS}")),
        "{reason}"
    );
}

#[test]
fn a_tray_saved_on_the_old_light_ground_opens_on_todays_mid_grey() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("old-ground");
    let mut s = painted_session(&gpu);
    let texels = (TRAY_WIDTH * TRAY_HEIGHT * 4) as usize;
    let today = s.tray.read_f32(&gpu, StateTexture::DryRefl0).unwrap()[0];
    for which in [StateTexture::DryRefl0, StateTexture::DryRefl1] {
        s.tray.write_f32(&gpu, which, &vec![0.75; texels]);
    }
    let path = dir.file("old-ground.cadmium");
    document::save(&gpu, &mut s, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    for which in [StateTexture::DryRefl0, StateTexture::DryRefl1] {
        let ground = opened.tray.read_f32(&gpu, which).unwrap();
        assert!(ground.iter().all(|band| *band == today), "{which:?}");
    }
    assert!((today - 0.4).abs() < 1e-3, "{today}");
}
