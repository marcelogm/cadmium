use cadmium_engine::{
    canvas::CanvasSpec,
    document::DocumentSnapshot,
    gpu::GpuContext,
    palette::{ColourRef, MAX_MY_COLOURS, Palette, default_circle, paint_recipe},
    session::PaintSession,
};

use crate::entries::{Entries, format_reason, read_entries};
use crate::{TempDir, blank_sheet, open_painting, painted_session, painted_spec, save_painting};

#[test]
fn a_painting_reopens_with_its_pinned_colours_and_its_circle() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("palette");
    let (mut s, saved) = edited_palette(&gpu);
    save_painting(&gpu, &mut s, &dir.file("p.cadmium"));
    assert_eq!(open_painting(&gpu, &dir.file("p.cadmium")).palette, saved);
}

#[test]
fn a_new_painting_starts_with_no_pinned_colour_and_the_default_circle() {
    let gpu = GpuContext::new().unwrap();
    let s = painted_session(&gpu);
    assert!(s.palette.my_colours.is_empty());
    assert_eq!(
        s.palette.circle,
        default_circle(&s.db, &s.canvas.spec.pigment_slots)
    );
    assert_eq!(s.palette.circle, [ColourRef::Paint(0)]);
}

#[test]
fn a_palette_pointing_outside_the_painting_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("bad-palette");
    let path = dir.file("p.cadmium");
    save_painting(&gpu, &mut painted_session(&gpu), &path);
    let entries = Entries::saved(&path).edit_manifest(|manifest| {
        manifest["palette"]["circle"] = serde_json::json!([{ "Paint": 7 }]);
    });
    let reason = format_reason(read_entries(entries));
    assert!(reason.contains("palette"), "{reason}");
}

#[test]
fn a_painting_with_more_pinned_colours_than_the_limit_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("crowded-palette");
    let path = dir.file("p.cadmium");
    save_painting(&gpu, &mut painted_session(&gpu), &path);
    let entries = Entries::saved(&path).edit_manifest(|manifest| {
        manifest["palette"]["my_colours"] =
            serde_json::json!(vec![paint_recipe(0); MAX_MY_COLOURS + 1]);
    });
    let reason = format_reason(read_entries(entries));
    assert_eq!(
        reason,
        "palette: palette holds 65 pinned colours, the limit is 64"
    );
}

#[test]
fn a_painting_without_a_palette_is_a_format_error() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("no-palette");
    let path = dir.file("p.cadmium");
    save_painting(&gpu, &mut painted_session(&gpu), &path);
    let entries = Entries::saved(&path).edit_manifest(|manifest| {
        manifest.as_object_mut().unwrap().remove("palette");
    });
    let reason = format_reason(read_entries(entries));
    assert!(reason.contains("palette"), "{reason}");
}

fn edited_palette(gpu: &GpuContext) -> (PaintSession, Palette) {
    let mut s = painted_session(gpu);
    s.palette.pin(paint_recipe(1)).unwrap();
    s.palette.circle = vec![ColourRef::Mine(0), ColourRef::Paint(0)];
    let edited = s.palette.clone();
    (s, edited)
}

#[test]
fn a_snapshot_restored_into_a_running_session_brings_its_palette() {
    let gpu = GpuContext::new().unwrap();
    let (mut saved, palette) = edited_palette(&gpu);
    let mut saved_sheet = blank_sheet(&gpu, &saved);
    let snapshot = DocumentSnapshot::capture(&gpu, &mut saved, &mut saved_sheet).unwrap();
    let mut running = painted_session(&gpu);
    let mut running_sheet = blank_sheet(&gpu, &running);
    snapshot
        .restore_into(&gpu, &mut running, &mut running_sheet)
        .unwrap();
    assert_eq!(running.palette, palette);
}

#[test]
fn replacing_the_document_starts_the_palette_of_the_new_painting() {
    let gpu = GpuContext::new().unwrap();
    let (mut s, _) = edited_palette(&gpu);
    let spec = CanvasSpec {
        pigment_slots: vec!["titanium_white".into(), "burnt_sienna".into()],
        ..painted_spec()
    };
    s.replace_document(&gpu, spec).unwrap();
    assert!(s.palette.my_colours.is_empty());
    assert_eq!(s.palette.circle, [ColourRef::Paint(1)]);
}

#[test]
fn adding_a_paint_to_the_painting_leaves_the_palette_as_it_was() {
    let gpu = GpuContext::new().unwrap();
    let (mut s, palette) = edited_palette(&gpu);
    s.add_slot(&gpu, "ultramarine_blue").unwrap();
    assert_eq!(s.palette, palette);
}
