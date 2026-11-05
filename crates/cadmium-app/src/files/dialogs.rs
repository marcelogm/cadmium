use std::path::{Path, PathBuf};

use adw::prelude::*;
use gtk::{gio, glib};

use super::title::{UNTITLED, document_name};
use super::{IMAGE_EXTENSION, PAINTING_EXTENSION};
use crate::LOG_DOMAIN;
use crate::dialogs::overwrite_confirmed;
use crate::window::CadmiumWindow;

const PAINTING_FILTER_NAME: &str = "Cadmium painting (*.cadmium)";
const IMAGE_FILTER_NAME: &str = "PNG image (*.png)";
const PAINTING_MIME_TYPE: &str = "application/x-cadmium";
const IMAGE_MIME_TYPE: &str = "image/png";
const OPEN_TITLE: &str = "Open Painting";
const SAVE_AS_TITLE: &str = "Save Painting";
const EXPORT_TITLE: &str = "Export PNG";
const NOT_A_LOCAL_FILE: &str = "The chosen file is not local";

fn overwrite_needs_confirmation(chosen: &Path, target: &Path, target_exists: bool) -> bool {
    chosen != target && target_exists
}

fn with_extension(path: PathBuf, extension: &str) -> PathBuf {
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

impl CadmiumWindow {
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
        glib::spawn_future_local(save_chosen_painting(self.downgrade(), dialog));
    }

    pub fn open(&self) {
        let file = self.state().borrow().file.clone();
        let dialog = file_dialog(OPEN_TITLE, &painting_filter(), file.as_deref());
        glib::spawn_future_local(open_chosen_painting(self.downgrade(), dialog));
    }

    pub fn export(&self) {
        let file = self.state().borrow().file.clone();
        let dialog = file_dialog(EXPORT_TITLE, &image_filter(), file.as_deref());
        dialog.set_initial_name(Some(&format!(
            "{}.{IMAGE_EXTENSION}",
            document_name(file.as_deref())
        )));
        glib::spawn_future_local(export_chosen_image(self.downgrade(), dialog));
    }

    fn chosen_path(&self, chosen: Result<gio::File, glib::Error>) -> Option<PathBuf> {
        match chosen {
            Ok(file) => self.local_path(&file),
            Err(error) if was_dismissed(&error) => None,
            Err(error) => {
                glib::g_warning!(LOG_DOMAIN, "file dialog failed: {error}");
                self.show_toast(&error.to_string());
                None
            }
        }
    }

    fn local_path(&self, file: &gio::File) -> Option<PathBuf> {
        let path = file.path();
        if path.is_none() {
            self.show_toast(NOT_A_LOCAL_FILE);
        }
        path
    }
}

fn was_dismissed(error: &glib::Error) -> bool {
    error.matches(gtk::DialogError::Dismissed) || error.matches(gtk::DialogError::Cancelled)
}

async fn save_chosen_painting(window: glib::WeakRef<CadmiumWindow>, dialog: gtk::FileDialog) {
    let Some(target) = chosen_write_target(&window, dialog, PAINTING_EXTENSION).await else {
        return;
    };
    let saving = window.upgrade().map(|window| window.save_to(target));
    if let Some(saving) = saving {
        saving.await;
    }
}

async fn open_chosen_painting(window: glib::WeakRef<CadmiumWindow>, dialog: gtk::FileDialog) {
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
}

async fn export_chosen_image(window: glib::WeakRef<CadmiumWindow>, dialog: gtk::FileDialog) {
    let Some(target) = chosen_write_target(&window, dialog, IMAGE_EXTENSION).await else {
        return;
    };
    let exporting = window.upgrade().map(|window| window.export_to(target));
    if let Some(exporting) = exporting {
        exporting.await;
    }
}

async fn chosen_write_target(
    window: &glib::WeakRef<CadmiumWindow>,
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
mod tests;
