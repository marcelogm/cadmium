//! The brush editor smoke step: the editor held still with the brush mid-stroke.

use adw::prelude::*;
use cadmium_studio::{LOOP_PAUSE_MS, QUANTUM_MS, SETTLE_MS};
use gtk::glib;

use crate::LOG_DOMAIN;
use crate::brush_editor;
use crate::studio_view::StudioView;
use crate::window::CadmiumWindow;

/// The play toggle's icon while the studio plays.
const PAUSE_ICON: &str = "media-playback-pause-symbolic";

/// Opens the editor, pauses it through its play toggle as a user would, and
/// replays its loop to half the stroke. An editor that cannot open or replay
/// ends the run: its screenshot would lie.
pub(super) fn open_mid_stroke(window: &CadmiumWindow) {
    brush_editor::open(window);
    let Some(editor) = window.editor() else {
        fail("the brush editor did not open");
    };
    let (Some(view), Some(play)) = (
        find_widget::<StudioView>(editor.upcast_ref(), |_| true),
        find_widget::<gtk::ToggleButton>(editor.upcast_ref(), |button| {
            button.icon_name().is_some_and(|icon| icon == PAUSE_ICON)
        }),
    ) else {
        fail("the brush editor has no studio view or play toggle");
    };
    play.set_active(false);
    let state = window.state();
    let replayed = {
        let mut state = state.borrow_mut();
        let gpu = state.gpu.clone();
        match state.studio_mut() {
            Some(studio) if studio.player.is_playing() => fail("the play toggle did not pause"),
            Some(studio) => {
                let mid_stroke = mid_stroke_ms(studio.player.loop_ms());
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
    find_widget(window.editor()?.upcast_ref(), |_| true)
}

/// The first widget of type `W` under `widget`, itself included, that `wanted` accepts.
fn find_widget<W: IsA<gtk::Widget>>(
    widget: &gtk::Widget,
    wanted: impl Fn(&W) -> bool + Copy,
) -> Option<W> {
    if let Some(found) = widget.downcast_ref::<W>()
        && wanted(found)
    {
        return Some(found.clone());
    }
    let mut next = widget.first_child();
    while let Some(child) = next {
        if let Some(found) = find_widget(&child, wanted) {
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
