use std::path::{Path, PathBuf};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};
use oleo_engine::document::{DocumentError, DocumentSnapshot};

use crate::LOG_DOMAIN;
use crate::dialogs::{painting_discard_confirmed, unsaved_changes_discard_confirmed};
use crate::window::OleoWindow;

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
        match self.state().borrow().file.clone() {
            Some(path) => {
                glib::spawn_future_local(self.clone().save_to(path));
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
        let window = self.clone();
        glib::spawn_future_local(async move {
            let chosen = dialog.save_future(Some(&window)).await;
            if let Some(path) = window.chosen_path(chosen) {
                window
                    .save_to(with_extension(path, PAINTING_EXTENSION))
                    .await;
            }
        });
    }

    pub fn open(&self) {
        let file = self.state().borrow().file.clone();
        let dialog = file_dialog(OPEN_TITLE, &painting_filter(), file.as_deref());
        let window = self.clone();
        glib::spawn_future_local(async move {
            let chosen = dialog.open_future(Some(&window)).await;
            if let Some(path) = window.chosen_path(chosen) {
                window.open_path(path).await;
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
        let window = self.clone();
        glib::spawn_future_local(async move {
            let chosen = dialog.save_future(Some(&window)).await;
            if let Some(path) = window.chosen_path(chosen) {
                window
                    .export_to(with_extension(path, IMAGE_EXTENSION))
                    .await;
            }
        });
    }

    pub async fn save_to(self, path: PathBuf) -> bool {
        if !self.imp().saves_in_flight.borrow_mut().insert(path.clone()) {
            glib::g_debug!(LOG_DOMAIN, "already saving to {}", path.display());
            return false;
        }
        let generation = self.imp().document_generation.get();
        let (snapshot, was_dirty) = {
            let state = self.state();
            let mut state = state.borrow_mut();
            let snapshot = state.capture_document();
            (snapshot, std::mem::take(&mut state.dirty_since_save))
        };
        self.canvas().queue_present();
        let target = path.clone();
        let written = finished(gio::spawn_blocking(move || snapshot.write(&target)).await);
        self.imp().saves_in_flight.borrow_mut().remove(&path);
        let same_document = generation == self.imp().document_generation.get();
        let saved = match written {
            Ok(()) => {
                if same_document {
                    self.state().borrow_mut().file = Some(path);
                    self.show_document_title();
                }
                self.show_toast(SAVED_TOAST);
                true
            }
            Err(failure) => {
                if same_document {
                    self.state().borrow_mut().dirty_since_save |= was_dirty;
                }
                glib::g_warning!(
                    LOG_DOMAIN,
                    "saving {} failed: {}",
                    path.display(),
                    failure.detail
                );
                self.show_toast(&format!("{CANNOT_SAVE}: {}", failure.shown));
                false
            }
        };
        self.close_if_it_waited_for_saves();
        saved
    }

    pub async fn open_path(self, path: PathBuf) -> bool {
        let source = path.clone();
        let snapshot =
            match finished(gio::spawn_blocking(move || DocumentSnapshot::read(&source)).await) {
                Ok(snapshot) => snapshot,
                Err(failure) => {
                    glib::g_warning!(
                        LOG_DOMAIN,
                        "opening {} failed: {}",
                        path.display(),
                        failure.detail
                    );
                    self.show_toast(&format!("{CANNOT_OPEN}: {}", failure.shown));
                    return false;
                }
            };
        let dirty = self.state().borrow().dirty_since_save;
        if dirty && !painting_discard_confirmed(&self).await {
            return false;
        }
        self.state().borrow_mut().open_painting(snapshot, path);
        self.after_document_replaced();
        true
    }

    pub async fn export_to(self, path: PathBuf) -> bool {
        let image = self.state().borrow_mut().render_export();
        self.canvas().queue_present();
        let target = path.clone();
        match finished(gio::spawn_blocking(move || image.write_png(&target)).await) {
            Ok(()) => {
                self.show_toast(EXPORTED_TOAST);
                true
            }
            Err(failure) => {
                glib::g_warning!(
                    LOG_DOMAIN,
                    "exporting {} failed: {}",
                    path.display(),
                    failure.detail
                );
                self.show_toast(&format!("{CANNOT_EXPORT}: {}", failure.shown));
                false
            }
        }
    }

    pub fn may_close(&self) -> bool {
        let imp = self.imp();
        if !imp.saves_in_flight.borrow().is_empty() {
            imp.close_after_saves.set(true);
            return false;
        }
        if imp.close_confirmed.get() || !self.state().borrow().dirty_since_save {
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

    fn close_if_it_waited_for_saves(&self) {
        let imp = self.imp();
        if imp.saves_in_flight.borrow().is_empty() && imp.close_after_saves.replace(false) {
            self.close();
        }
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
    fn chosen_name_with_the_extension_in_any_case_is_kept() {
        assert_eq!(
            with_extension(PathBuf::from("/tmp/estudo.OLEO"), PAINTING_EXTENSION),
            PathBuf::from("/tmp/estudo.OLEO")
        );
    }
}
