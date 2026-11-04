use std::path::{Path, PathBuf};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};
use oleo_engine::document::{DocumentError, DocumentSnapshot};

use crate::LOG_DOMAIN;
use crate::dialogs::{
    ask_to_quit_while_writing, overwrite_confirmed, painting_discard_confirmed,
    unsaved_changes_discard_confirmed,
};
use crate::window::OleoWindow;
use crate::writes::{WriteKind, work_would_be_lost};

pub const PAINTING_EXTENSION: &str = "oleo";
pub const IMAGE_EXTENSION: &str = "png";
const APPLICATION_NAME: &str = "Óleo";
const UNTITLED: &str = "Sem título";
const PAINTING_FILTER_NAME: &str = "Pintura Óleo (*.oleo)";
const IMAGE_FILTER_NAME: &str = "Imagem PNG (*.png)";
const PAINTING_MIME_TYPE: &str = "application/x-oleo";
const IMAGE_MIME_TYPE: &str = "image/png";
const OPEN_TITLE: &str = "Abrir pintura";
const SAVE_AS_TITLE: &str = "Salvar pintura";
const EXPORT_TITLE: &str = "Exportar PNG";
const SAVED_TOAST: &str = "Salvo";
const SAVING_TOAST: &str = "Salvando…";
const EXPORTING_TOAST: &str = "Exportando…";
const WRITE_HANG_SECONDS: u32 = 10;
const EXPORTED_TOAST: &str = "Exportado";
const CANNOT_OPEN: &str = "Não foi possível abrir";
const CANNOT_SAVE: &str = "Não foi possível salvar";
const CANNOT_EXPORT: &str = "Não foi possível exportar";
const NOT_A_LOCAL_FILE: &str = "o arquivo escolhido não é local";
const BACKGROUND_TASK_FAILED: &str = "a tarefa em segundo plano falhou";
const FILE_NOT_FOUND: &str = "o arquivo não existe";
const PERMISSION_DENIED: &str = "sem permissão para acessar o arquivo";
const ALREADY_EXISTS: &str = "o arquivo já existe";
const STORAGE_FULL: &str = "não há espaço livre no disco";
const READ_ONLY: &str = "o local é somente leitura";
const UNREADABLE_DATA: &str = "o arquivo está corrompido ou incompleto";
const INPUT_OUTPUT_FAILED: &str = "falha de leitura ou gravação";
const NOT_A_PAINTING: &str = "o arquivo não é uma pintura do Óleo válida";

struct Failure {
    shown: String,
    detail: String,
}

pub fn describe_document_error(error: &DocumentError) -> String {
    match error {
        DocumentError::Io(error) => describe_io_error(error.kind()).to_string(),
        DocumentError::Format(_) => NOT_A_PAINTING.to_string(),
        DocumentError::UnsupportedVersion(version) => {
            format!("a pintura usa o formato {version}, que esta versão do Óleo não sabe ler")
        }
    }
}

fn describe_io_error(kind: std::io::ErrorKind) -> &'static str {
    match kind {
        std::io::ErrorKind::NotFound => FILE_NOT_FOUND,
        std::io::ErrorKind::PermissionDenied => PERMISSION_DENIED,
        std::io::ErrorKind::AlreadyExists => ALREADY_EXISTS,
        std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded => STORAGE_FULL,
        std::io::ErrorKind::ReadOnlyFilesystem => READ_ONLY,
        std::io::ErrorKind::InvalidData | std::io::ErrorKind::UnexpectedEof => UNREADABLE_DATA,
        _ => INPUT_OUTPUT_FAILED,
    }
}

pub fn document_name(file: Option<&Path>) -> String {
    file.and_then(Path::file_stem)
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| UNTITLED.to_string())
}

pub fn window_title(file: Option<&Path>) -> String {
    match file {
        Some(_) => format!("{} — {APPLICATION_NAME}", document_name(file)),
        None => APPLICATION_NAME.to_string(),
    }
}

pub fn overwrite_needs_confirmation(chosen: &Path, target: &Path, target_exists: bool) -> bool {
    chosen != target && target_exists
}

pub fn with_extension(path: PathBuf, extension: &str) -> PathBuf {
    if path
        .extension()
        .is_some_and(|current| current.eq_ignore_ascii_case(extension))
    {
        return path;
    }
    let mut named = path.into_os_string();
    named.push(".");
    named.push(extension);
    PathBuf::from(named)
}

fn file_filter(name: &str, extension: &str, mime_type: &str) -> gtk::FileFilter {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(name));
    filter.add_suffix(extension);
    filter.add_mime_type(mime_type);
    filter
}

fn painting_filter() -> gtk::FileFilter {
    file_filter(PAINTING_FILTER_NAME, PAINTING_EXTENSION, PAINTING_MIME_TYPE)
}

fn image_filter() -> gtk::FileFilter {
    file_filter(IMAGE_FILTER_NAME, IMAGE_EXTENSION, IMAGE_MIME_TYPE)
}

fn file_dialog(title: &str, filter: &gtk::FileFilter, current: Option<&Path>) -> gtk::FileDialog {
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(filter);
    let dialog = gtk::FileDialog::builder()
        .title(title)
        .modal(true)
        .filters(&filters)
        .default_filter(filter)
        .build();
    if let Some(folder) = current.and_then(Path::parent) {
        dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
    }
    dialog
}

fn finished<T>(outcome: std::thread::Result<Result<T, DocumentError>>) -> Result<T, Failure> {
    match outcome {
        Ok(result) => result.map_err(|error| Failure {
            shown: describe_document_error(&error),
            detail: error.to_string(),
        }),
        Err(_) => Err(Failure {
            shown: BACKGROUND_TASK_FAILED.to_string(),
            detail: BACKGROUND_TASK_FAILED.to_string(),
        }),
    }
}

impl OleoWindow {
    pub fn save(&self) {
        let file = self.state().borrow().file.clone();
        match file {
            Some(path) => {
                glib::spawn_future_local(self.save_to(path));
            }
            None => self.save_as(),
        }
    }

    pub fn save_as(&self) {
        let file = self.state().borrow().file.clone();
        let dialog = file_dialog(SAVE_AS_TITLE, &painting_filter(), file.as_deref());
        match &file {
            Some(path) => dialog.set_initial_file(Some(&gio::File::for_path(path))),
            None => dialog.set_initial_name(Some(&format!("{UNTITLED}.{PAINTING_EXTENSION}"))),
        }
        let window = self.downgrade();
        glib::spawn_future_local(async move {
            let Some(target) = chosen_write_target(&window, dialog, PAINTING_EXTENSION).await
            else {
                return;
            };
            let saving = window.upgrade().map(|window| window.save_to(target));
            if let Some(saving) = saving {
                saving.await;
            }
        });
    }

    pub fn open(&self) {
        let file = self.state().borrow().file.clone();
        let dialog = file_dialog(OPEN_TITLE, &painting_filter(), file.as_deref());
        let window = self.downgrade();
        glib::spawn_future_local(async move {
            let Some(choosing) = window
                .upgrade()
                .map(|parent| dialog.open_future(Some(&parent)))
            else {
                return;
            };
            let chosen = choosing.await;
            let opening = window.upgrade().and_then(|window| {
                let path = window.chosen_path(chosen)?;
                Some(window.open_path(path))
            });
            if let Some(opening) = opening {
                opening.await;
            }
        });
    }

    pub fn export(&self) {
        let file = self.state().borrow().file.clone();
        let dialog = file_dialog(EXPORT_TITLE, &image_filter(), file.as_deref());
        dialog.set_initial_name(Some(&format!(
            "{}.{IMAGE_EXTENSION}",
            document_name(file.as_deref())
        )));
        let window = self.downgrade();
        glib::spawn_future_local(async move {
            let Some(target) = chosen_write_target(&window, dialog, IMAGE_EXTENSION).await else {
                return;
            };
            let exporting = window.upgrade().map(|window| window.export_to(target));
            if let Some(exporting) = exporting {
                exporting.await;
            }
        });
    }

    pub fn save_to(&self, path: PathBuf) -> impl Future<Output = bool> + use<> {
        let generation = self.imp().document_generation.get();
        let captured = self.begin_write(&path, WriteKind::Painting).then(|| {
            let state = self.state();
            let mut state = state.borrow_mut();
            (state.capture_document(), state.edit_mark())
        });
        if captured.is_some() {
            self.canvas().queue_present();
        }
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
            window.state().borrow_mut().open_painting(snapshot, path);
            window.after_document_replaced();
            true
        }
    }

    pub fn export_to(&self, path: PathBuf) -> impl Future<Output = bool> + use<> {
        let image = self
            .begin_write(&path, WriteKind::Image)
            .then(|| self.state().borrow_mut().render_export());
        if image.is_some() {
            self.canvas().queue_present();
        }
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

    pub fn work_would_be_lost(&self) -> bool {
        let unsaved_edits = self.state().borrow().has_unsaved_edits();
        work_would_be_lost(unsaved_edits, &self.imp().writes_in_flight.borrow())
    }

    pub fn may_close(&self) -> bool {
        let imp = self.imp();
        if imp.abandon_writes.get() {
            return true;
        }
        if !imp.writes_in_flight.borrow().is_empty() {
            imp.close_after_writes.set(true);
            self.offer_to_quit_if_writes_hang();
            return false;
        }
        if imp.close_confirmed.get() || !self.work_would_be_lost() {
            return true;
        }
        if !imp.asking_to_close.replace(true) {
            let window = self.clone();
            glib::spawn_future_local(async move {
                let discard = unsaved_changes_discard_confirmed(&window).await;
                window.imp().asking_to_close.set(false);
                if discard {
                    window.imp().close_confirmed.set(true);
                    window.close();
                }
            });
        }
        false
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

    fn offer_to_quit_if_writes_hang(&self) {
        let imp = self.imp();
        if imp.write_wait_timeout.borrow().is_some() || imp.write_wait_alert.borrow().is_some() {
            return;
        }
        let window = self.downgrade();
        let timeout = glib::timeout_add_seconds_local_once(WRITE_HANG_SECONDS, move || {
            let Some(window) = window.upgrade() else {
                return;
            };
            let imp = window.imp();
            imp.write_wait_timeout.take();
            if imp.close_after_writes.get() && !imp.writes_in_flight.borrow().is_empty() {
                window.ask_to_quit_while_writing();
            }
        });
        imp.write_wait_timeout.replace(Some(timeout));
    }

    fn ask_to_quit_while_writing(&self) {
        let window = self.downgrade();
        let alert = ask_to_quit_while_writing(self, move || {
            if let Some(window) = window.upgrade() {
                window.imp().abandon_writes.set(true);
                window.close();
            }
        });
        let window = self.downgrade();
        alert.connect_closed(move |_| {
            if let Some(window) = window.upgrade() {
                window.imp().write_wait_alert.take();
            }
        });
        self.imp().write_wait_alert.replace(Some(alert));
    }

    fn chosen_path(&self, chosen: Result<gio::File, glib::Error>) -> Option<PathBuf> {
        match chosen {
            Ok(file) => {
                let path = file.path();
                if path.is_none() {
                    self.show_toast(NOT_A_LOCAL_FILE);
                }
                path
            }
            Err(error)
                if error.matches(gtk::DialogError::Dismissed)
                    || error.matches(gtk::DialogError::Cancelled) =>
            {
                None
            }
            Err(error) => {
                glib::g_warning!(LOG_DOMAIN, "file dialog failed: {error}");
                self.show_toast(&error.to_string());
                None
            }
        }
    }

    pub fn show_document_title(&self) {
        let title = window_title(self.state().borrow().file.as_deref());
        self.set_title(Some(&title));
    }
}

async fn chosen_write_target(
    window: &glib::WeakRef<OleoWindow>,
    dialog: gtk::FileDialog,
    extension: &str,
) -> Option<PathBuf> {
    let choosing = dialog.save_future(Some(&window.upgrade()?));
    let chosen = choosing.await;
    let parent = window.upgrade()?;
    let chosen = parent.chosen_path(chosen)?;
    let target = with_extension(chosen.clone(), extension);
    if overwrite_needs_confirmation(&chosen, &target, target.exists()) {
        let file_name = target.file_name()?.to_string_lossy().into_owned();
        if !overwrite_confirmed(&parent, &file_name).await {
            return None;
        }
    }
    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untitled_painting_is_named_sem_titulo() {
        assert_eq!(document_name(None), "Sem título");
        assert_eq!(window_title(None), "Óleo");
    }

    #[test]
    fn saved_painting_titles_the_window_with_its_name() {
        let file = Path::new("/home/pintor/Retrato de Ana.oleo");
        assert_eq!(document_name(Some(file)), "Retrato de Ana");
        assert_eq!(window_title(Some(file)), "Retrato de Ana — Óleo");
    }

    #[test]
    fn chosen_name_without_the_extension_gets_it() {
        assert_eq!(
            with_extension(PathBuf::from("/tmp/estudo"), PAINTING_EXTENSION),
            PathBuf::from("/tmp/estudo.oleo")
        );
        assert_eq!(
            with_extension(PathBuf::from("/tmp/estudo.v2"), IMAGE_EXTENSION),
            PathBuf::from("/tmp/estudo.v2.png")
        );
    }

    #[test]
    fn document_errors_read_in_portuguese() {
        let missing = DocumentError::Io(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert_eq!(describe_document_error(&missing), FILE_NOT_FOUND);
        let full = DocumentError::Io(std::io::Error::from(std::io::ErrorKind::StorageFull));
        assert_eq!(describe_document_error(&full), STORAGE_FULL);
        let broken = DocumentError::Format("missing manifest.json".into());
        assert_eq!(describe_document_error(&broken), NOT_A_PAINTING);
        assert_eq!(
            describe_document_error(&DocumentError::UnsupportedVersion(7)),
            "a pintura usa o formato 7, que esta versão do Óleo não sabe ler"
        );
        let other = DocumentError::Io(std::io::Error::other("boom"));
        assert_eq!(describe_document_error(&other), INPUT_OUTPUT_FAILED);
    }

    #[test]
    fn appended_extension_landing_on_an_existing_file_asks_before_replacing_it() {
        let chosen = Path::new("/tmp/estudo");
        let target = with_extension(chosen.to_path_buf(), PAINTING_EXTENSION);
        assert!(overwrite_needs_confirmation(chosen, &target, true));
    }

    #[test]
    fn appended_extension_on_a_new_name_writes_without_asking() {
        let chosen = Path::new("/tmp/estudo");
        let target = with_extension(chosen.to_path_buf(), PAINTING_EXTENSION);
        assert!(!overwrite_needs_confirmation(chosen, &target, false));
    }

    #[test]
    fn name_already_confirmed_by_the_file_chooser_is_not_asked_again() {
        let chosen = Path::new("/tmp/estudo.oleo");
        let target = with_extension(chosen.to_path_buf(), PAINTING_EXTENSION);
        assert!(!overwrite_needs_confirmation(chosen, &target, true));
    }

    #[test]
    fn chosen_name_with_the_extension_in_any_case_is_kept() {
        assert_eq!(
            with_extension(PathBuf::from("/tmp/estudo.OLEO"), PAINTING_EXTENSION),
            PathBuf::from("/tmp/estudo.OLEO")
        );
    }
}
