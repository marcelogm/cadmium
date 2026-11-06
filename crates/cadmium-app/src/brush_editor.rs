//! The brush editor: the worn brush painting its demo stroke in 3D, beside a
//! read-only summary of the brush.

use std::rc::Rc;

use adw::prelude::*;
use cadmium_engine::brush::BrushSpec;
use cadmium_studio::{DemoStroke, SLOW_MOTION, StudioError, hair_name, head_name};
use gtk::glib;

use crate::state::{AppState, SharedState, StudioSession};
use crate::studio_view::StudioView;
use crate::window::CadmiumWindow;

mod build;

/// What the summary calls a head or hair that is no longer a factory one.
const CUSTOM: &str = "Custom";
const NORMAL_SPEED: f64 = 1.0;
const FAILURE_TOAST: &str = "Can't show the brush studio";

/// One press on the editor's toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolbarAction {
    Play(bool),
    SlowMotion(bool),
    /// The stroke dropdown's index into `DemoStroke::ALL`.
    Stroke(usize),
    ResetView,
}

/// Runs a toolbar action on the open studio and shows its result.
type Runner = Rc<dyn Fn(ToolbarAction)>;

/// Opens the studio on the worn brush and shows it in the editor; a studio
/// that cannot open is toasted instead.
pub fn open(window: &CadmiumWindow) {
    if window.has_editor() {
        return;
    }
    let state = window.state();
    if !open_studio(&state) {
        window.after_studio_failure();
        return;
    }
    let (brush, stroke) = {
        let state = state.borrow();
        let stroke = state
            .studio()
            .map_or(DemoStroke::ALL[0], |s| s.player.stroke());
        (state.worn_brush_spec(), stroke)
    };
    let view = StudioView::new(state);
    window.follow_studio_view(&view);
    let dialog = build::dialog(&brush, &view, stroke_index(stroke), runner(window, &view));
    dialog.connect_closed(glib::clone!(
        #[weak]
        window,
        move |_| window.after_editor_closed()
    ));
    window.keep_editor(&dialog);
    dialog.present(Some(window));
}

fn open_studio(state: &SharedState) -> bool {
    let mut state = state.borrow_mut();
    state.open_studio();
    state.studio_open()
}

/// Applies each action, then presents a frame: a paused studio has no tick of
/// its own. An action that closed the studio closes the editor.
fn runner(window: &CadmiumWindow, view: &StudioView) -> Runner {
    let (window, view) = (window.downgrade(), view.downgrade());
    Rc::new(move |action| {
        let (Some(window), Some(view)) = (window.upgrade(), view.upgrade()) else {
            return;
        };
        let state = window.state();
        let still_open = {
            let mut state = state.borrow_mut();
            apply(&mut state, action);
            state.studio_open()
        };
        if still_open {
            view.queue_present();
        } else {
            window.after_studio_failure();
        }
    })
}

fn apply(state: &mut AppState, action: ToolbarAction) {
    match action {
        ToolbarAction::Stroke(index) => {
            if let Some(&stroke) = DemoStroke::ALL.get(index) {
                state.set_studio_stroke(stroke);
            }
        }
        ToolbarAction::Play(true) => change_studio(state, |studio| studio.player.play()),
        ToolbarAction::Play(false) => change_studio(state, |studio| studio.player.pause()),
        ToolbarAction::SlowMotion(slow) => {
            change_studio(state, |studio| studio.player.set_speed(speed(slow)));
        }
        ToolbarAction::ResetView => change_studio(state, |studio| studio.camera.reset()),
    }
}

fn change_studio(state: &mut AppState, change: impl FnOnce(&mut StudioSession)) {
    if let Some(studio) = state.studio_mut() {
        change(studio);
    }
}

fn speed(slow_motion: bool) -> f64 {
    if slow_motion {
        SLOW_MOTION
    } else {
        NORMAL_SPEED
    }
}

/// The stroke dropdown's entries, in `DemoStroke::ALL` order.
fn stroke_labels() -> Vec<&'static str> {
    DemoStroke::ALL.into_iter().map(DemoStroke::label).collect()
}

fn stroke_index(stroke: DemoStroke) -> usize {
    DemoStroke::ALL
        .iter()
        .position(|&s| s == stroke)
        .unwrap_or(0)
}

/// The read-only rows describing `brush`, as (title, value).
fn summary_rows(brush: &BrushSpec) -> [(&'static str, String); 4] {
    [
        ("Name", brush.name.clone()),
        ("Head", head_name(&brush.head).unwrap_or(CUSTOM).to_owned()),
        ("Hair", hair_name(&brush.hair).unwrap_or(CUSTOM).to_owned()),
        ("Size", format!("{:.0} px", brush.size_px)),
    ]
}

/// The toast for a studio that closed with `error`; none for a lost GPU, which
/// the GPU-lost dialog announces.
pub fn failure_toast(error: Option<&StudioError>, gpu_lost: bool) -> Option<String> {
    match error? {
        StudioError::Gpu(_) => None,
        _ if gpu_lost => None,
        error => Some(format!("{FAILURE_TOAST}: {error}")),
    }
}

#[cfg(test)]
mod tests;
