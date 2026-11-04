use std::path::PathBuf;

use adw::prelude::*;
use cadmium_engine::document::DocumentSnapshot;
use gtk::{gio, glib};

use super::{OUT_OF_VIDEO_MEMORY, finished};
use crate::dialogs::painting_discard_confirmed;
use crate::window::CadmiumWindow;

const CANNOT_OPEN: &str = "Não foi possível abrir";

impl CadmiumWindow {
    pub fn open_path(&self, path: PathBuf) -> impl Future<Output = bool> + use<> {
        read_painting_into(self.downgrade(), path)
    }
}

async fn read_painting_into(window: glib::WeakRef<CadmiumWindow>, path: PathBuf) -> bool {
    let source = path.clone();
    let read = finished(gio::spawn_blocking(move || DocumentSnapshot::read(&source)).await);
    let Some(window) = window.upgrade() else {
        return false;
    };
    let snapshot = match read {
        Ok(snapshot) => snapshot,
        Err(failure) => {
            window.report_failure("opening", &path, CANNOT_OPEN, &failure);
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
