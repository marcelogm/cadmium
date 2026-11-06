//! The brush editor smoke step: the editor held still with the brush mid-stroke.

use adw::prelude::*;
use cadmium_studio::{LOOP_PAUSE_MS, QUANTUM_MS, SETTLE_MS};
use gtk::glib;

use crate::LOG_DOMAIN;
use crate::brush_editor;
use crate::studio_view::StudioView;
use crate::window::CadmiumWindow;

/// Opens the editor, replays its loop to half the stroke and pauses there.
/// An editor that cannot open or replay ends the run: its screenshot would lie.
pub(super) fn open_mid_stroke(window: &CadmiumWindow) {
    brush_editor::open(window);
    let Some(view) = studio_view(window) else {
        fail("the brush editor did not open");
    };
    let state = window.state();
    let replayed = {
        let mut state = state.borrow_mut();
        let gpu = state.gpu.clone();
        match state.studio_mut() {
            Some(studio) => {
                let mid_stroke = mid_stroke_ms(studio.player.loop_ms());
                studio.player.pause();
                studio.player.run_until(&gpu, mid_stroke)
            }
            None => fail("the brush editor has no studio"),
        }
    };
    if let Err(error) = replayed {
        fail(&format!(
            "the brush editor could not replay its stroke: {error}"
        ));
    }
    view.queue_present();
}

/// Half-way through the pen's travel: a loop is the stroke, the bristles'
/// settle, one closing quantum and the pause.
pub(super) fn mid_stroke_ms(loop_ms: f64) -> f64 {
    ((loop_ms - LOOP_PAUSE_MS - SETTLE_MS - QUANTUM_MS) / 2.0).max(0.0)
}

/// Whether the open editor's studio has shown a frame; true with no editor.
pub(super) fn presented_or_closed(window: &CadmiumWindow) -> bool {
    studio_view(window).is_none_or(|view| view.presented_frames() > 0)
}

fn studio_view(window: &CadmiumWindow) -> Option<StudioView> {
    find_studio_view(window.editor()?.upcast_ref())
}

fn find_studio_view(widget: &gtk::Widget) -> Option<StudioView> {
    if let Some(view) = widget.downcast_ref::<StudioView>() {
        return Some(view.clone());
    }
    let mut next = widget.first_child();
    while let Some(child) = next {
        if let Some(found) = find_studio_view(&child) {
            return Some(found);
        }
        next = child.next_sibling();
    }
    None
}

fn fail(reason: &str) -> ! {
    glib::g_critical!(LOG_DOMAIN, "smoke brush editor: {reason}");
    std::process::exit(1);
}
