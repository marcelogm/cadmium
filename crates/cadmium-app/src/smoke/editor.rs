//! The brush editor smoke step: the editor held still with the brush mid-stroke.

use adw::prelude::*;
use gtk::glib;

use crate::LOG_DOMAIN;
use crate::brush_editor::{self, PLAY_BUTTON_NAME};
use crate::studio_view::StudioView;
use crate::window::CadmiumWindow;

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
            button.widget_name() == PLAY_BUTTON_NAME
        }),
    ) else {
        fail("the brush editor has no studio view or play toggle");
    };
    if !play.is_active() {
        fail("the brush editor opened paused");
    }
    // Before the state borrow below: the toggle's handler borrows the state itself.
    play.set_active(false);
    let state = window.state();
    let replayed = {
        let mut state = state.borrow_mut();
        let gpu = state.gpu.clone();
        match state.studio_mut() {
            Some(studio) if studio.player.is_playing() => fail("the play toggle did not pause"),
            Some(studio) if !studio.player.is_ready() => fail("the studio is not ready"),
            Some(studio) => {
                let mid_stroke = studio.player.stroke_ms() / 2.0;
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

/// Whether the screenshot can be taken: with an editor expected, once its
/// studio has shown a frame. An editor that closed ends the run: the
/// screenshot would lie.
pub(super) fn ready_for_screenshot(window: &CadmiumWindow, expects_editor: bool) -> bool {
    if !expects_editor {
        return true;
    }
    match studio_view(window) {
        Some(view) => view.presented_frames() > 0,
        None => fail("the brush editor closed before the screenshot"),
    }
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
