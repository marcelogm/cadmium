use std::path::{Path, PathBuf};

use adw::prelude::*;
use adw::subclass::prelude::*;
use cadmium_engine::document::{DocumentSnapshot, ExportedImage};
use cadmium_engine::gpu::GpuLost;
use gtk::{gio, glib};

use super::finished;
use crate::LOG_DOMAIN;
use crate::window::CadmiumWindow;
use crate::writes::{EditMark, WriteKind};

const SAVED_TOAST: &str = "Salvo";
const SAVING_TOAST: &str = "Salvando…";
const EXPORTING_TOAST: &str = "Exportando…";
const EXPORTED_TOAST: &str = "Exportado";
const CANNOT_SAVE: &str = "Não foi possível salvar";
const CANNOT_EXPORT: &str = "Não foi possível exportar";

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
        self.show_toast(SAVED_TOAST);
    }

    fn begin_write(&self, path: &Path, kind: WriteKind) -> bool {
        if self.imp().writes_in_flight.borrow_mut().begin(path, kind) {
            return true;
        }
        glib::g_debug!(LOG_DOMAIN, "already writing {}", path.display());
        self.show_toast(match kind {
            WriteKind::Painting => SAVING_TOAST,
            WriteKind::Image => EXPORTING_TOAST,
        });
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
        imp.writes_in_flight.borrow_mut().finish(path);
        if !imp.writes_in_flight.borrow().is_empty() {
            return;
        }
        if let Some(timeout) = imp.write_wait_timeout.take() {
            timeout.remove();
        }
        if let Some(alert) = imp.write_wait_alert.take() {
            alert.force_close();
        }
        if imp.close_after_writes.replace(false) {
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
    let target = path.clone();
    let written = finished(gio::spawn_blocking(move || snapshot.write(&target)).await);
    let Some(window) = window.upgrade() else {
        return written.is_ok();
    };
    let same_document = generation == window.imp().document_generation.get();
    let saved = match written {
        Ok(()) => {
            window.painting_saved(&path, mark, same_document);
            true
        }
        Err(failure) => {
            window.report_failure("saving", &path, CANNOT_SAVE, &failure);
            false
        }
    };
    window.finish_write(&path);
    saved
}

async fn write_image(
    window: glib::WeakRef<CadmiumWindow>,
    path: PathBuf,
    image: Option<ExportedImage>,
) -> bool {
    let Some(image) = image else {
        return false;
    };
    let target = path.clone();
    let written = finished(gio::spawn_blocking(move || image.write_png(&target)).await);
    let Some(window) = window.upgrade() else {
        return written.is_ok();
    };
    let exported = match written {
        Ok(()) => {
            window.show_toast(EXPORTED_TOAST);
            true
        }
        Err(failure) => {
            window.report_failure("exporting", &path, CANNOT_EXPORT, &failure);
            false
        }
    };
    window.finish_write(&path);
    exported
}
