//! The brush editor's smoke steps as the environment asks for them.

use std::ffi::OsString;
use std::path::PathBuf;

use super::{BRUSH, BRUSH_EDITOR, BrushStep, NEW_BRUSH, SHEET_STROKE};
use crate::smoke::config::{SCREENSHOT, TRAY};
use crate::smoke::steps::SmokeStep;
use crate::smoke::{SETTLE_FRAMES_AFTER_FILE_WORK, SmokeConfig};

fn lookup<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    }
}

#[test]
fn brush_editor_alone_starts_a_smoke_run_that_opens_it() {
    let config = SmokeConfig::from_lookup(lookup(&[(BRUSH_EDITOR, "60")])).unwrap();
    assert_eq!(config.brushes.editor, Some(60));
    assert_eq!(
        config.steps(),
        [SmokeStep::Brush(BrushStep::Editor(60)), SmokeStep::Quit]
    );
}

#[test]
fn brush_editor_opens_after_the_tray_and_before_the_screenshot() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (SCREENSHOT, "/tmp/shot.png"),
        (BRUSH_EDITOR, "60"),
        (TRAY, "1"),
        (BRUSH, "Round · Synthetic"),
    ]))
    .unwrap();
    assert_eq!(
        config.steps(),
        [
            SmokeStep::Brush(BrushStep::Pick("Round · Synthetic".to_string())),
            SmokeStep::ShowTray,
            SmokeStep::Brush(BrushStep::Editor(60)),
            SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png")),
            SmokeStep::Quit,
        ]
    );
}

#[test]
fn unparsable_brush_editor_frames_open_no_editor() {
    assert_eq!(
        SmokeConfig::from_lookup(lookup(&[(BRUSH_EDITOR, "soon")])),
        None
    );
}

#[test]
fn the_brush_editor_holds_the_end_back_by_its_frames() {
    let editor = SmokeStep::Brush(BrushStep::Editor(60));
    let save = SmokeStep::Save(PathBuf::from("/tmp/a.cadmium"));
    let screenshot = SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png"));
    assert_eq!(editor.settle_frames(), Some(60));
    assert_eq!(save.settle_frames(), Some(SETTLE_FRAMES_AFTER_FILE_WORK));
    assert_eq!(screenshot.settle_frames(), None);
    assert!(editor.opens_editor());
    assert!(!save.opens_editor() && !screenshot.opens_editor());
}

#[test]
fn a_sheet_stroke_follows_the_brush_editor() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (BRUSH_EDITOR, "60"),
        (SHEET_STROKE, "1"),
        (SCREENSHOT, "/tmp/shot.png"),
    ]))
    .unwrap();
    assert_eq!(
        config.steps(),
        [
            SmokeStep::Brush(BrushStep::Editor(60)),
            SmokeStep::Brush(BrushStep::SheetStroke),
            SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png")),
            SmokeStep::Quit,
        ]
    );
}

#[test]
fn a_sheet_stroke_without_the_editor_paints_nothing() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (SHEET_STROKE, "1"),
        (SCREENSHOT, "/tmp/shot.png"),
    ]))
    .unwrap();
    assert!(
        !config
            .steps()
            .contains(&SmokeStep::Brush(BrushStep::SheetStroke))
    );
}

#[test]
fn a_new_brush_editor_opens_in_place_of_the_worn_brush_one() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (BRUSH_EDITOR, "60"),
        (NEW_BRUSH, "1"),
        (SHEET_STROKE, "1"),
    ]))
    .unwrap();
    let editor = SmokeStep::Brush(BrushStep::NewBrushEditor(60));
    assert_eq!(
        config.steps(),
        [
            editor.clone(),
            SmokeStep::Brush(BrushStep::SheetStroke),
            SmokeStep::Quit,
        ]
    );
    assert!(editor.opens_editor());
    assert_eq!(editor.settle_frames(), Some(60));
}

#[test]
fn a_new_brush_without_the_editor_opens_nothing() {
    let config = SmokeConfig::from_lookup(lookup(&[(NEW_BRUSH, "1")])).unwrap();
    assert_eq!(config.steps(), [SmokeStep::Quit]);
}
