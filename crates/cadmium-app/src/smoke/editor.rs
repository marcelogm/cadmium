//! The brush editor smoke step: the editor on show for the screenshot.

use adw::prelude::*;
use gtk::glib;

use super::steps::smoke_stroke;
use crate::LOG_DOMAIN;
use crate::brush_editor;
use crate::window::CadmiumWindow;

const NEW_BRUSH_ACTION: &str = "win.new-brush";

/// How much of the editor the screenshot step can see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorShown {
    Closed,
    Unmapped,
    /// Mapped, its example strokes or its test sheet still being painted.
    Painting,
    Mapped,
}

/// What the screenshot step does about the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Readiness {
    Ready,
    Wait,
    /// The editor the run opened is gone: the screenshot would lie.
    EditorClosed,
}

/// Opens the brush editor. An editor that does not open ends the run: its
/// screenshot would lie.
pub(super) fn open(window: &CadmiumWindow) {
    brush_editor::open(window);
    expect_editor(window);
}

/// Opens the brush editor on a new brush through the rack menu's action, as "New Brush…"
/// does. An editor that does not open ends the run.
pub(super) fn open_new(window: &CadmiumWindow) {
    if let Err(error) = WidgetExt::activate_action(window, NEW_BRUSH_ACTION, None) {
        fail(&format!("could not run {NEW_BRUSH_ACTION}: {error}"));
    }
    expect_editor(window);
}

fn expect_editor(window: &CadmiumWindow) {
    if !window.has_editor() {
        fail("the brush editor did not open");
    }
}

/// Whether the screenshot can be taken: with an editor expected, once its
/// dialog is mapped, its example strokes are shown and its test sheet is at
/// rest. A smoke run turns animations off, so a mapped dialog is already at
/// rest; the step's frame hold (`Editor(frames)` extends `earliest_end_frame`)
/// only lets its contents draw. An editor that closed ends the run.
pub(super) fn ready_for_screenshot(window: &CadmiumWindow, expects_editor: bool) -> bool {
    match readiness(expects_editor, shown(window)) {
        Readiness::Ready => true,
        Readiness::Wait => false,
        Readiness::EditorClosed => fail("the brush editor closed before the screenshot"),
    }
}

/// What the screenshot step does given whether an editor is expected and how much of it is shown.
fn readiness(expects_editor: bool, shown: EditorShown) -> Readiness {
    match (expects_editor, shown) {
        (false, _) | (true, EditorShown::Mapped) => Readiness::Ready,
        (true, EditorShown::Unmapped | EditorShown::Painting) => Readiness::Wait,
        (true, EditorShown::Closed) => Readiness::EditorClosed,
    }
}

/// `Unmapped`: kept by the window but not yet presented.
fn shown(window: &CadmiumWindow) -> EditorShown {
    match window.editor() {
        None => EditorShown::Closed,
        Some(editor) if !editor.is_mapped() => EditorShown::Unmapped,
        Some(editor) if window.examples_pending() || sheet_busy(&editor) => EditorShown::Painting,
        Some(_) => EditorShown::Mapped,
    }
}

/// Whether the editor's test sheet still presents frames: its first, or a stroke's.
fn sheet_busy(editor: &adw::Dialog) -> bool {
    brush_editor::test_sheet_in(editor).is_some_and(|sheet| sheet.is_presenting())
}

/// Whether the editor's test sheet shows its first frame, ready for a stroke. An editor
/// that closed ends the run.
pub(super) fn sheet_shown(window: &CadmiumWindow) -> bool {
    match window.editor() {
        None => fail("the brush editor closed before its test stroke"),
        Some(editor) => {
            editor.is_mapped()
                && brush_editor::test_sheet_in(&editor).is_some_and(|sheet| sheet.has_frame())
        }
    }
}

/// Paints the smoke stroke across the editor's test sheet as the card shows it (the canvas
/// overflows the card; a stroke laid out on the canvas could fall outside it), as a hand's
/// stroke is painted.
pub(super) fn paint_on_sheet(window: &CadmiumWindow) {
    let sheet = window
        .editor()
        .and_then(|editor| brush_editor::test_sheet_in(&editor));
    let Some(sheet) = sheet else {
        fail("the brush editor has no test sheet");
    };
    let (width, height) = (sheet.width().max(1) as u32, sheet.height().max(1) as u32);
    sheet.paint_stroke(&smoke_stroke(width, height));
}

fn fail(reason: &str) -> ! {
    glib::g_critical!(LOG_DOMAIN, "smoke brush editor: {reason}");
    std::process::exit(1);
}

#[cfg(test)]
mod tests;
