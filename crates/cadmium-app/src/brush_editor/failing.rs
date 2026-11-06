//! Giving up on the editor when its studio work fails: the editor closes and a toast says
//! why; a lost GPU goes to the window's GPU-lost dialog instead.

use std::rc::Rc;

use cadmium_studio::StudioError;
use gtk::glib;

use super::Editor;
use super::closing::Closing;
use crate::LOG_DOMAIN;
use crate::window::CadmiumWindow;

/// The toast for `what` failing with `error`; `None` for a lost GPU, which the window's
/// GPU-lost dialog reports.
pub(super) fn failure_toast(what: &str, error: &StudioError, gpu_lost: bool) -> Option<String> {
    match error {
        StudioError::Gpu(_) => None,
        _ if gpu_lost => None,
        error => Some(format!("{what}: {error}")),
    }
}

impl Editor {
    /// Closes the editor and says why `what` failed. A lost GPU goes to the window's
    /// GPU-lost dialog, which closes the editor first (once: a later failure closes it here).
    pub(super) fn give_up(self: &Rc<Self>, what: &str, error: &StudioError) {
        let window = self.window.upgrade();
        let gpu_lost = window
            .as_ref()
            .is_some_and(|window| window.state().borrow().gpu_is_lost());
        let toast = failure_toast(what, error, gpu_lost);
        if toast.is_none()
            && let Some(window) = &window
        {
            window.announce_gpu_lost();
        }
        self.finish(Closing::Done);
        if let (Some(window), Some(toast)) = (window, toast) {
            warn_and_toast(&window, &toast);
        }
    }
}

/// Says why `what` failed before the editor opened: a toast, or the GPU-lost dialog.
pub(super) fn report(window: &CadmiumWindow, what: &str, error: &StudioError) {
    let gpu_lost = window.state().borrow().gpu_is_lost();
    match failure_toast(what, error, gpu_lost) {
        Some(toast) => warn_and_toast(window, &toast),
        None => window.announce_gpu_lost(),
    }
}

fn warn_and_toast(window: &CadmiumWindow, toast: &str) {
    glib::g_warning!(LOG_DOMAIN, "{toast}");
    window.show_toast(toast);
}

#[cfg(test)]
mod tests;
