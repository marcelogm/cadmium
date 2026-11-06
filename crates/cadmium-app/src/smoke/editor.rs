//! The brush editor smoke step: the editor on show for the screenshot.

use adw::prelude::*;
use gtk::glib;

use crate::LOG_DOMAIN;
use crate::brush_editor;
use crate::window::CadmiumWindow;

/// How much of the editor the screenshot step can see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorShown {
    Closed,
    Unmapped,
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
    if !window.has_editor() {
        fail("the brush editor did not open");
    }
}

/// Whether the screenshot can be taken: with an editor expected, once its
/// dialog is mapped. The editor holds no GPU view in this stage, and a smoke
/// run turns animations off, so a mapped dialog is already at rest; the step's
/// frame hold (`Editor(frames)` extends `earliest_end_frame`) only lets its
/// contents draw. An editor that closed ends the run.
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
        (true, EditorShown::Unmapped) => Readiness::Wait,
        (true, EditorShown::Closed) => Readiness::EditorClosed,
    }
}

/// `Unmapped`: kept by the window but not yet presented.
fn shown(window: &CadmiumWindow) -> EditorShown {
    match window.editor() {
        None => EditorShown::Closed,
        Some(editor) if editor.is_mapped() => EditorShown::Mapped,
        Some(_) => EditorShown::Unmapped,
    }
}

fn fail(reason: &str) -> ! {
    glib::g_critical!(LOG_DOMAIN, "smoke brush editor: {reason}");
    std::process::exit(1);
}

#[cfg(test)]
mod tests;
