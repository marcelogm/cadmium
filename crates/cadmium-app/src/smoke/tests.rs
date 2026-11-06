use std::ffi::OsString;

use super::brushes::{BRUSH, BRUSH_DELETE, BRUSH_DIR, BRUSH_EXPORT, BRUSH_IMPORT, BrushStep};
use super::config::{
    DEFAULT_EXIT_AFTER_FRAMES, DIALOG, DMABUF_SELFTEST, EXIT_AFTER_FRAMES, EXPORT, OPEN, PAGE,
    PRESENT_MODE, SAVE, SCREENSHOT, STROKE, TOOL, TRAY,
};
use super::steps::{
    STROKE_PRESSURE_END, STROKE_PRESSURE_START, STROKE_SAMPLES, SmokeDialog, SmokeStep,
    smoke_stroke,
};
use super::*;

fn lookup<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    }
}

#[test]
fn no_smoke_variables_means_no_smoke_run() {
    assert_eq!(SmokeConfig::from_lookup(lookup(&[])), None);
}

#[test]
fn screenshot_alone_exits_after_default_frame_count() {
    let config = SmokeConfig::from_lookup(lookup(&[(SCREENSHOT, "/tmp/shot.png")])).unwrap();
    assert_eq!(config.exit_after_frames, DEFAULT_EXIT_AFTER_FRAMES);
    assert_eq!(config.screenshot, Some(PathBuf::from("/tmp/shot.png")));
}

#[test]
fn explicit_frame_count_is_honoured() {
    let config = SmokeConfig::from_lookup(lookup(&[(EXIT_AFTER_FRAMES, "60")])).unwrap();
    assert_eq!(config.exit_after_frames, 60);
    assert_eq!(config.screenshot, None);
}

#[test]
fn unparsable_frame_count_falls_back_to_default() {
    let config = SmokeConfig::from_lookup(lookup(&[(EXIT_AFTER_FRAMES, "soon")])).unwrap();
    assert_eq!(config.exit_after_frames, DEFAULT_EXIT_AFTER_FRAMES);
}

#[test]
fn dialog_alone_starts_a_smoke_run_that_opens_it() {
    let config = SmokeConfig::from_lookup(lookup(&[(DIALOG, "new")])).unwrap();
    assert_eq!(config.dialog, Some(SmokeDialog::NewPainting));
    assert_eq!(config.exit_after_frames, DEFAULT_EXIT_AFTER_FRAMES);
}

#[test]
fn about_and_shortcuts_dialogs_are_recognised() {
    let about = SmokeConfig::from_lookup(lookup(&[(DIALOG, "about")])).unwrap();
    let shortcuts = SmokeConfig::from_lookup(lookup(&[(DIALOG, "shortcuts")])).unwrap();
    assert_eq!(about.dialog, Some(SmokeDialog::About));
    assert_eq!(shortcuts.dialog, Some(SmokeDialog::Shortcuts));
}

#[test]
fn empty_screenshot_path_counts_as_unset() {
    assert_eq!(SmokeConfig::from_lookup(lookup(&[(SCREENSHOT, "")])), None);
}

#[test]
fn stroke_runs_after_the_dialog_and_before_the_screenshot_and_quit() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (STROKE, "1"),
        (DIALOG, "about"),
        (SCREENSHOT, "/tmp/shot.png"),
    ]))
    .unwrap();
    assert_eq!(
        config.steps(),
        [
            SmokeStep::OpenDialog(SmokeDialog::About),
            SmokeStep::PaintStroke,
            SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png")),
            SmokeStep::Quit,
        ]
    );
}

#[test]
fn page_is_shown_after_the_stroke_and_before_the_screenshot() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (PAGE, "lab"),
        (STROKE, "1"),
        (SCREENSHOT, "/tmp/shot.png"),
    ]))
    .unwrap();
    assert_eq!(
        config.steps(),
        [
            SmokeStep::PaintStroke,
            SmokeStep::ShowPage("lab".to_string()),
            SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png")),
            SmokeStep::Quit,
        ]
    );
}

#[test]
fn file_steps_run_after_the_stroke_and_before_the_page_and_screenshot() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (OPEN, "/tmp/a.cadmium"),
        (EXPORT, "/tmp/a.png"),
        (PAGE, "lab"),
        (SAVE, "/tmp/a.cadmium"),
        (STROKE, "1"),
        (SCREENSHOT, "/tmp/shot.png"),
    ]))
    .unwrap();
    assert_eq!(
        config.steps(),
        [
            SmokeStep::PaintStroke,
            SmokeStep::Save(PathBuf::from("/tmp/a.cadmium")),
            SmokeStep::Open(PathBuf::from("/tmp/a.cadmium")),
            SmokeStep::Export(PathBuf::from("/tmp/a.png")),
            SmokeStep::ShowPage("lab".to_string()),
            SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png")),
            SmokeStep::Quit,
        ]
    );
}

#[test]
fn tray_opens_after_the_page_and_before_the_screenshot() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (TRAY, "1"),
        (PAGE, "lab"),
        (SCREENSHOT, "/tmp/shot.png"),
    ]))
    .unwrap();
    assert_eq!(
        config.steps(),
        [
            SmokeStep::ShowPage("lab".to_string()),
            SmokeStep::ShowTray,
            SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png")),
            SmokeStep::Quit,
        ]
    );
}

#[test]
fn tray_zero_does_not_open_the_tray() {
    assert_eq!(SmokeConfig::from_lookup(lookup(&[(TRAY, "0")])), None);
}

#[test]
fn tool_is_chosen_before_anything_else() {
    let config =
        SmokeConfig::from_lookup(lookup(&[(STROKE, "1"), (TOOL, "knife"), (DIALOG, "about")]))
            .unwrap();
    assert_eq!(
        config.steps(),
        [
            SmokeStep::ChooseTool(UiTool::Paint(cadmium_engine::stroke::Tool::Knife)),
            SmokeStep::OpenDialog(SmokeDialog::About),
            SmokeStep::PaintStroke,
            SmokeStep::Quit,
        ]
    );
}

#[test]
fn unknown_tool_is_ignored() {
    let config = SmokeConfig::from_lookup(lookup(&[(TOOL, "spray")])).unwrap();
    assert_eq!(config.tool, None);
}

#[test]
fn open_alone_starts_a_smoke_run() {
    let config = SmokeConfig::from_lookup(lookup(&[(OPEN, "/tmp/a.cadmium")])).unwrap();
    assert_eq!(config.open, Some(PathBuf::from("/tmp/a.cadmium")));
}

#[test]
fn page_alone_starts_a_smoke_run() {
    let config = SmokeConfig::from_lookup(lookup(&[(PAGE, "lab")])).unwrap();
    assert_eq!(config.page.as_deref(), Some("lab"));
}

#[test]
fn stroke_zero_does_not_paint() {
    assert_eq!(SmokeConfig::from_lookup(lookup(&[(STROKE, "0")])), None);
}

#[test]
fn smoke_stroke_crosses_the_canvas_middle_with_a_pressure_ramp() {
    let samples = smoke_stroke(2048, 1536);
    assert_eq!(samples.len(), STROKE_SAMPLES);
    assert!((samples[0].pressure - STROKE_PRESSURE_START).abs() < 1e-6);
    assert!((samples[STROKE_SAMPLES - 1].pressure - STROKE_PRESSURE_END).abs() < 1e-6);
    let peak = samples.iter().map(|s| s.pressure).fold(0.0, f32::max);
    assert!(peak > 0.95);
    assert!(samples.iter().all(|s| s.x > 0.0 && s.x < 2048.0));
    assert!(
        samples
            .iter()
            .all(|s| s.y > 1536.0 * 0.3 && s.y < 1536.0 * 0.7)
    );
    assert!(
        samples
            .windows(2)
            .all(|pair| pair[1].time_ms > pair[0].time_ms)
    );
}

#[test]
fn dmabuf_selftest_alone_starts_a_smoke_run() {
    let config = SmokeConfig::from_lookup(lookup(&[(DMABUF_SELFTEST, "1")])).unwrap();
    assert_eq!(config.dmabuf_selftest, Some(ModifierPolicy::Any));
}

#[test]
fn linear_present_mode_restricts_the_dmabuf_selftest_to_linear() {
    let config =
        SmokeConfig::from_lookup(lookup(&[(DMABUF_SELFTEST, "1"), (PRESENT_MODE, "linear")]))
            .unwrap();
    assert_eq!(config.dmabuf_selftest, Some(ModifierPolicy::LinearOnly));
}

#[test]
fn nodcc_present_mode_keeps_the_dmabuf_selftest_off_compressed_modifiers() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (DMABUF_SELFTEST, "1"),
        (PRESENT_MODE, "dmabuf-nodcc"),
    ]))
    .unwrap();
    assert_eq!(config.dmabuf_selftest, Some(ModifierPolicy::NoCompression));
}

#[test]
fn unknown_dialog_is_ignored() {
    let config = SmokeConfig::from_lookup(lookup(&[(DIALOG, "preferences")])).unwrap();
    assert_eq!(config.dialog, None);
}

#[test]
fn a_brush_folder_alone_starts_a_smoke_run_that_loads_it() {
    let config = SmokeConfig::from_lookup(lookup(&[(BRUSH_DIR, "/tmp/brushes")])).unwrap();
    assert_eq!(config.brush_dir(), Some(PathBuf::from("/tmp/brushes")));
    assert_eq!(config.steps(), [SmokeStep::Quit]);
}

#[test]
fn without_a_brush_folder_the_smoke_library_stays_factory_only() {
    let config = SmokeConfig::from_lookup(lookup(&[(STROKE, "1")])).unwrap();
    assert_eq!(config.brush_dir(), None);
}

#[test]
fn brush_steps_wrap_the_stroke_and_file_steps() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (BRUSH_DELETE, "1"),
        (BRUSH_EXPORT, "/tmp/out.cadbrush"),
        (SCREENSHOT, "/tmp/shot.png"),
        (EXPORT, "/tmp/a.png"),
        (STROKE, "1"),
        (BRUSH, "Soft Flat"),
        (BRUSH_IMPORT, "/tmp/in.cadbrush"),
        (PAGE, "lab"),
    ]))
    .unwrap();
    assert_eq!(
        config.steps(),
        [
            SmokeStep::Brush(BrushStep::Import(PathBuf::from("/tmp/in.cadbrush"))),
            SmokeStep::Brush(BrushStep::Pick("Soft Flat".to_string())),
            SmokeStep::PaintStroke,
            SmokeStep::Export(PathBuf::from("/tmp/a.png")),
            SmokeStep::Brush(BrushStep::Export(PathBuf::from("/tmp/out.cadbrush"))),
            SmokeStep::Brush(BrushStep::Delete),
            SmokeStep::ShowPage("lab".to_string()),
            SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png")),
            SmokeStep::Quit,
        ]
    );
}

#[test]
fn brush_delete_zero_and_empty_brush_values_do_nothing() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (BRUSH_DELETE, "0"),
        (BRUSH, ""),
        (BRUSH_IMPORT, ""),
    ]));
    assert_eq!(config, None);
}
