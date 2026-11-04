use std::path::PathBuf;

use adw::prelude::*;
use cadmium_engine::document::DocumentSnapshot;
use gtk::{gio, glib};

use super::{OUT_OF_VIDEO_MEMORY, finished};
use crate::LOG_DOMAIN;
use crate::dialogs::painting_discard_confirmed;
use crate::window::CadmiumWindow;

const CANNOT_OPEN: &str = "Não foi possível abrir";

impl CadmiumWindow {
    #[expect(clippy::excessive_nesting)]
    pub fn open_path(&self, path: PathBuf) -> impl Future<Output = bool> + use<> {
        let window = self.downgrade();
        async move {
            let source = path.clone();
            let read = finished(gio::spawn_blocking(move || DocumentSnapshot::read(&source)).await);
            let Some(window) = window.upgrade() else {
                return false;
            };
            let snapshot = match read {
                Ok(snapshot) => snapshot,
                Err(failure) => {
                    glib::g_warning!(
                        LOG_DOMAIN,
                        "opening {} failed: {}",
                        path.display(),
                        failure.detail
                    );
                    window.show_toast(&format!("{CANNOT_OPEN}: {}", failure.shown));
                    return false;
                }
            };
            if window.work_would_be_lost() && !painting_discard_confirmed(&window).await {
                return false;
            }
            let opened = window.state().borrow_mut().open_painting(snapshot, path);
            window.after_document_replaced();
            if opened.is_err() {
                window.show_toast(&format!("{CANNOT_OPEN}: {OUT_OF_VIDEO_MEMORY}"));
            }
            opened.is_ok()
        }
    }
}
