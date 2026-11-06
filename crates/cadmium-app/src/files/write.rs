use std::path::{Path, PathBuf};

use adw::prelude::*;
use adw::subclass::prelude::*;
use cadmium_engine::document::{DocumentError, DocumentSnapshot, ExportedImage};
use cadmium_engine::gpu::GpuLost;
use gtk::{gio, glib};

use super::{finished, write_wording};
use crate::LOG_DOMAIN;
use crate::window::CadmiumWindow;
use crate::writes::{AfterWrite, EditMark, WriteKind};

impl CadmiumWindow {
    pub fn save_to(&self, path: PathBuf) -> impl Future<Output = bool> + use<> {
        let generation = self.imp().document_generation.get();
        let captured = self
            .begin_write(&path, WriteKind::Painting)
            .then(|| self.capture_painting());
        let captured = self.settle_capture(&path, captured);
        write_painting(self.downgrade(), path, generation, captured)
    }

    pub fn export_to(&self, path: PathBuf) -> impl Future<Output = bool> + use<> {
        let image = self
            .begin_write(&path, WriteKind::Image)
            .then(|| self.state().borrow_mut().render_export());
        let image = self.settle_capture(&path, image);
        write_image(self.downgrade(), path, image)
    }

    fn capture_painting(&self) -> Result<(DocumentSnapshot, EditMark), GpuLost> {
        let state = self.state();
        let mut state = state.borrow_mut();
        let mark = state.edit_mark();
        state.capture_document().map(|snapshot| (snapshot, mark))
    }

    fn painting_saved(&self, path: &Path, mark: EditMark, same_document: bool) {
        if same_document {
            self.state().borrow_mut().saved_to(path.to_path_buf(), mark);
            self.show_document_title();
        }
        self.show_toast(write_wording(WriteKind::Painting).done);
    }

    fn begin_write(&self, path: &Path, kind: WriteKind) -> bool {
        if self.imp().closing.borrow_mut().begin_write(path, kind) {
            return true;
        }
        glib::g_debug!(LOG_DOMAIN, "already writing {}", path.display());
        self.show_toast(write_wording(kind).busy);
        false
    }

    fn settle_capture<T>(&self, path: &Path, captured: Option<Result<T, GpuLost>>) -> Option<T> {
        let captured = captured?;
        self.canvas().queue_present();
        match captured {
            Ok(captured) => Some(captured),
            Err(_) => {
                self.finish_write(path);
                None
            }
        }
    }

    fn finish_write(&self, path: &Path) {
        let imp = self.imp();
        let after = imp.closing.borrow_mut().finish_write(path);
        if after == AfterWrite::StillWriting {
            return;
        }
        if let Some(timeout) = imp.write_wait_timeout.take() {
            timeout.remove();
        }
        if let Some(alert) = imp.write_wait_alert.take() {
            alert.force_close();
        }
        if after == AfterWrite::CloseNow {
            self.close();
        }
    }
}

async fn write_painting(
    window: glib::WeakRef<CadmiumWindow>,
    path: PathBuf,
    generation: u64,
    captured: Option<(DocumentSnapshot, EditMark)>,
) -> bool {
    let Some((snapshot, mark)) = captured else {
        return false;
    };
    let write = move |target: &Path| snapshot.write(target);
    let saved = move |window: &CadmiumWindow, path: &Path| {
        let same_document = generation == window.imp().document_generation.get();
        window.painting_saved(path, mark, same_document);
    };
    write_in_background(window, path, WriteKind::Painting, write, saved).await
}

async fn write_image(
    window: glib::WeakRef<CadmiumWindow>,
    path: PathBuf,
    image: Option<ExportedImage>,
) -> bool {
    let Some(image) = image else {
        return false;
    };
    let write = move |target: &Path| image.write_png(target);
    let exported =
        |window: &CadmiumWindow, _: &Path| window.show_toast(write_wording(WriteKind::Image).done);
    write_in_background(window, path, WriteKind::Image, write, exported).await
}

async fn write_in_background(
    window: glib::WeakRef<CadmiumWindow>,
    path: PathBuf,
    kind: WriteKind,
    write: impl FnOnce(&Path) -> Result<(), DocumentError> + Send + 'static,
    on_written: impl FnOnce(&CadmiumWindow, &Path),
) -> bool {
    let target = path.clone();
    let written = finished(gio::spawn_blocking(move || write(&target)).await);
    let Some(window) = window.upgrade() else {
        return written.is_ok();
    };
    let succeeded = match written {
        Ok(()) => {
            on_written(&window, &path);
            true
        }
        Err(failure) => {
            let wording = write_wording(kind);
            window.report_failure(wording.action, &path, wording.failed, &failure);
            false
        }
    };
    window.finish_write(&path);
    succeeded
}
