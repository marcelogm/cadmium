use std::path::{Path, PathBuf};

use adw::prelude::*;
use adw::subclass::prelude::*;
use cadmium_engine::gpu::GpuLost;
use gtk::{gio, glib};

use super::finished;
use crate::LOG_DOMAIN;
use crate::window::CadmiumWindow;
use crate::writes::WriteKind;

const SAVED_TOAST: &str = "Salvo";
const SAVING_TOAST: &str = "Salvando…";
const EXPORTING_TOAST: &str = "Exportando…";
const EXPORTED_TOAST: &str = "Exportado";
const CANNOT_SAVE: &str = "Não foi possível salvar";
const CANNOT_EXPORT: &str = "Não foi possível exportar";

impl CadmiumWindow {
    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
    pub fn save_to(&self, path: PathBuf) -> impl Future<Output = bool> + use<> {
        let generation = self.imp().document_generation.get();
        let captured = self.begin_write(&path, WriteKind::Painting).then(|| {
            let state = self.state();
            let mut state = state.borrow_mut();
            let mark = state.edit_mark();
            state.capture_document().map(|snapshot| (snapshot, mark))
        });
        let captured = self.settle_capture(&path, captured);
        let window = self.downgrade();
        async move {
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
                    if same_document {
                        window.state().borrow_mut().saved_to(path.clone(), mark);
                        window.show_document_title();
                    }
                    window.show_toast(SAVED_TOAST);
                    true
                }
                Err(failure) => {
                    glib::g_warning!(
                        LOG_DOMAIN,
                        "saving {} failed: {}",
                        path.display(),
                        failure.detail
                    );
                    window.show_toast(&format!("{CANNOT_SAVE}: {}", failure.shown));
                    false
                }
            };
            window.finish_write(&path);
            saved
        }
    }

    #[expect(clippy::excessive_nesting)]
    pub fn export_to(&self, path: PathBuf) -> impl Future<Output = bool> + use<> {
        let image = self
            .begin_write(&path, WriteKind::Image)
            .then(|| self.state().borrow_mut().render_export());
        let image = self.settle_capture(&path, image);
        let window = self.downgrade();
        async move {
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
                    glib::g_warning!(
                        LOG_DOMAIN,
                        "exporting {} failed: {}",
                        path.display(),
                        failure.detail
                    );
                    window.show_toast(&format!("{CANNOT_EXPORT}: {}", failure.shown));
                    false
                }
            };
            window.finish_write(&path);
            exported
        }
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
