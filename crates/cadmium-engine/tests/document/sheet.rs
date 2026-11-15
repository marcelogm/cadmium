use cadmium_engine::{
    canvas::StateTexture,
    document::{self, DocumentError, DocumentSnapshot},
    gpu::GpuContext,
    session::PaintSession,
};

use crate::entries::{Entries, format_reason, read_entries};
use crate::support::{session, stroke};
use crate::{TempDir, blank_sheet, painted_session, sample, save_painting};

const SHEET_WET_VOLUME: &str = "sheet/state/wet_vol.zst";
const SHEET_GRAPHITE: &str = "sheet/state/graphite.zst";

fn mixed_sheet(gpu: &GpuContext, painting: &PaintSession, row_y: f64) -> PaintSession {
    let mut sheet = blank_sheet(gpu, painting);
    sheet.brush.recipe = vec![(0, 1.0)];
    sheet.reload_brush(gpu);
    stroke(gpu, &mut sheet, row_y, 60.0, 600.0);
    assert!(sheet.canvas.wet_totals(gpu).unwrap().volume > 0.0);
    sheet
}

fn state_bytes(gpu: &GpuContext, s: &PaintSession) -> Vec<Vec<u8>> {
    StateTexture::PAINT_STATE
        .iter()
        .map(|which| s.canvas.read_bytes(gpu, *which).unwrap())
        .collect()
}

#[test]
fn a_painting_reopens_with_the_paint_left_on_its_mixing_sheet() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("sheet");
    let path = dir.file("p.cadmium");
    let mut painting = painted_session(&gpu);
    let mut sheet = mixed_sheet(&gpu, &painting, 200.0);
    document::save(&gpu, &mut painting, &mut sheet, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    assert_eq!(opened.sheet.canvas.spec, sheet.canvas.spec);
    assert_eq!(state_bytes(&gpu, &opened.sheet), state_bytes(&gpu, &sheet));
    assert_eq!(
        state_bytes(&gpu, &opened.painting),
        state_bytes(&gpu, &painting)
    );
}

#[test]
fn a_saved_painting_holds_a_whole_sheet_under_its_own_prefix() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("sheet-entries");
    let path = dir.file("p.cadmium");
    save_painting(&gpu, &mut painted_session(&gpu), &path);
    let names = Entries::saved(&path).names();
    let of_the_sheet = names
        .iter()
        .filter(|name| name.starts_with("sheet/state/"))
        .count();
    assert_eq!(of_the_sheet, StateTexture::PAINT_STATE.len());
    assert!(names.iter().any(|name| name == SHEET_GRAPHITE));
    assert!(names.iter().all(|name| {
        name == "manifest.json" || name.starts_with("state/") || name.starts_with("sheet/state/")
    }));
}

#[test]
fn a_painting_missing_part_of_its_sheet_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("no-sheet");
    let path = dir.file("p.cadmium");
    save_painting(&gpu, &mut painted_session(&gpu), &path);
    let mut entries = Entries::saved(&path);
    entries.0.retain(|(name, _)| name != SHEET_WET_VOLUME);
    let reason = format_reason(read_entries(entries));
    assert!(
        reason.contains(&format!("missing {SHEET_WET_VOLUME}")),
        "{reason}"
    );
}

#[test]
fn a_snapshot_restored_into_running_sessions_replaces_the_sheet_too() {
    let gpu = GpuContext::new().unwrap();
    let mut saved = painted_session(&gpu);
    let mut saved_sheet = mixed_sheet(&gpu, &saved, 200.0);
    let snapshot = DocumentSnapshot::capture(&gpu, &mut saved, &mut saved_sheet).unwrap();
    let mut running = session(&gpu);
    let mut running_sheet = mixed_sheet(&gpu, &running, 400.0);
    snapshot
        .restore_into(&gpu, &mut running, &mut running_sheet)
        .unwrap();
    assert_eq!(running_sheet.canvas.spec, saved_sheet.canvas.spec);
    assert_eq!(
        state_bytes(&gpu, &running_sheet),
        state_bytes(&gpu, &saved_sheet)
    );
    assert!(!running_sheet.history.can_undo());
}

#[test]
fn saving_right_after_a_stroke_on_the_sheet_ends_keeps_the_stroke() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("unflushed-sheet");
    let path = dir.file("p.cadmium");
    let mut painting = painted_session(&gpu);
    let mut sheet = blank_sheet(&gpu, &painting);
    sheet.brush.recipe = vec![(0, 1.0)];
    sheet.reload_brush(&gpu);
    sheet.begin_stroke(sample(60.0, 200.0, 0));
    sheet.add_samples(&[sample(600.0, 220.0, 200)]);
    sheet.end_stroke();
    document::save(&gpu, &mut painting, &mut sheet, &path).unwrap();
    let opened = document::open(&gpu, &path).unwrap();
    let volume = opened
        .sheet
        .canvas
        .read_f32(&gpu, StateTexture::WetVol)
        .unwrap();
    assert!(volume.iter().any(|&v| v > 0.0));
}

#[test]
fn a_sheet_that_is_not_the_paintings_is_refused_and_nothing_is_written() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("foreign-sheet");
    let path = dir.file("p.cadmium");
    let mut painting = painted_session(&gpu);
    let mut foreign = PaintSession::for_sheet(&gpu, &["titanium_white".to_string()]).unwrap();
    let result = document::save(&gpu, &mut painting, &mut foreign, &path);
    let Err(DocumentError::Format(reason)) = result else {
        panic!("expected a format error, got {result:?}");
    };
    assert!(reason.contains("mixing sheet"), "{reason}");
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
}
