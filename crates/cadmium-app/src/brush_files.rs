//! Import, export and delete brushes from the brush panel's menu.

use std::fmt::Display;
use std::path::Path;

use adw::prelude::*;
use cadmium_engine::brush::{BRUSH_EXTENSION, BrushSpec, brush_to_json};
use gtk::glib;

use crate::LOG_DOMAIN;
use crate::brush_library::{Imported, export_name, file_name_for};
use crate::dialogs::delete_brush_confirmed;
use crate::files::dialogs::{chosen_write_target, file_dialog, file_filter};
use crate::window::CadmiumWindow;

const BRUSH_FILTER_NAME: &str = "Cadmium brush (*.cadbrush)";
const BRUSH_MIME_TYPE: &str = "application/x-cadmium-brush";
const IMPORT_TITLE: &str = "Import Brush";
const EXPORT_TITLE: &str = "Export Brush";
const IMPORT_FAILED: &str = "Can't import brush";
const EXPORT_FAILED: &str = "Can't export brush";
const DELETE_FAILED: &str = "Can't delete brush";

/// The worn brush as it would be exported, and its library index when it is a library brush.
fn worn_brush_spec(window: &CadmiumWindow) -> (BrushSpec, Option<usize>) {
    let state = window.state();
    let state = state.borrow();
    let settings = &state.session.brush;
    let (name, index) = export_name(&state.brushes, state.worn_library_brush(), settings);
    let spec = BrushSpec {
        name,
        size_px: settings.geometry.max_diameter_px,
        head: settings.head,
        hair: settings.hair,
    };
    (spec, index)
}

fn brush_filter() -> gtk::FileFilter {
    file_filter(BRUSH_FILTER_NAME, BRUSH_EXTENSION, BRUSH_MIME_TYPE)
}

impl CadmiumWindow {
    pub fn import_brush(&self) {
        let dialog = file_dialog(IMPORT_TITLE, &brush_filter(), None);
        glib::spawn_future_local(import_chosen(self.downgrade(), dialog));
    }

    pub fn export_brush(&self) {
        let (spec, _) = worn_brush_spec(self);
        let dialog = file_dialog(EXPORT_TITLE, &brush_filter(), None);
        dialog.set_initial_name(Some(&file_name_for(&spec.name)));
        glib::spawn_future_local(export_chosen(self.downgrade(), dialog, spec));
    }

    pub fn delete_brush(&self) {
        let Some((name, index)) = self.deletable_worn_brush() else {
            return;
        };
        glib::spawn_future_local(delete_confirmed(self.downgrade(), name, index));
    }

    /// Imports the brush file at `path`, as the Import dialog does once a file is chosen.
    pub(crate) fn import_brush_file(&self, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(text) => self.import_brush_text(&text),
            Err(error) => self.report_brush_error(IMPORT_FAILED, &error),
        }
    }

    fn deletable_worn_brush(&self) -> Option<(String, usize)> {
        let (spec, index) = worn_brush_spec(self);
        let index = index.filter(|&index| self.state().borrow().brushes.is_user(index))?;
        Some((spec.name, index))
    }

    fn import_brush_text(&self, text: &str) {
        let state = self.state();
        let imported = state.borrow_mut().import_brush(text);
        let imported = match imported {
            Ok(imported) => imported,
            Err(error) => return self.report_brush_error(IMPORT_FAILED, &error),
        };
        let name = state
            .borrow()
            .brushes
            .get(imported.index)
            .map(|brush| brush.name.clone());
        let Some(name) = name else {
            return;
        };
        if let Some(error) = &imported.save_error {
            glib::g_warning!(LOG_DOMAIN, "couldn't save brush {name}: {error}");
        }
        self.after_library_change(&import_toast(&name, &imported));
    }

    fn write_brush_file(&self, target: &Path, spec: &BrushSpec) {
        match std::fs::write(target, brush_to_json(spec)) {
            Ok(()) => self.show_toast(&format!("Exported {}", spec.name)),
            Err(error) => self.report_brush_error(EXPORT_FAILED, &error),
        }
    }

    fn delete_library_brush(&self, name: &str, index: usize) {
        let state = self.state();
        let still_there = state
            .borrow()
            .brushes
            .get(index)
            .is_some_and(|brush| brush.name == name);
        if !still_there {
            return;
        }
        let deleted = state.borrow_mut().delete_library_brush(index);
        match deleted {
            Ok(()) => {
                self.after_library_change(&format!("Deleted {name}"));
            }
            Err(error) => {
                self.report_brush_error(DELETE_FAILED, &error);
                self.refresh_panels();
            }
        }
    }

    fn after_library_change(&self, toast: &str) {
        self.show_toast(toast);
        self.canvas().queue_draw();
        self.refresh_panels();
    }

    fn report_brush_error(&self, failed: &str, error: &dyn Display) {
        let message = format!("{failed}: {error}");
        glib::g_warning!(LOG_DOMAIN, "{message}");
        self.show_toast(&message);
    }
}

/// What the toast says after an import, from what happened to the brush.
fn import_toast(name: &str, imported: &Imported) -> String {
    let mut toast = format!("Imported {name}");
    if imported.renamed {
        toast.push_str(" (renamed)");
    }
    if imported.adjusted {
        toast.push_str("; some values were out of range and were adjusted");
    }
    if !imported.saved() {
        toast.push_str("; couldn't save it to your brush folder, it will be gone after you quit");
    }
    toast
}

async fn import_chosen(window: glib::WeakRef<CadmiumWindow>, dialog: gtk::FileDialog) {
    let Some(choosing) = window
        .upgrade()
        .map(|parent| dialog.open_future(Some(&parent)))
    else {
        return;
    };
    let chosen = choosing.await;
    let Some(window) = window.upgrade() else {
        return;
    };
    let Some(path) = window.chosen_path(chosen) else {
        return;
    };
    window.import_brush_file(&path);
}

async fn export_chosen(
    window: glib::WeakRef<CadmiumWindow>,
    dialog: gtk::FileDialog,
    spec: BrushSpec,
) {
    let Some(target) = chosen_write_target(&window, dialog, BRUSH_EXTENSION).await else {
        return;
    };
    let Some(window) = window.upgrade() else {
        return;
    };
    window.write_brush_file(&target, &spec);
}

async fn delete_confirmed(window: glib::WeakRef<CadmiumWindow>, name: String, index: usize) {
    let Some(parent) = window.upgrade() else {
        return;
    };
    if delete_brush_confirmed(&parent, &name).await {
        parent.delete_library_brush(&name, index);
    }
}

#[cfg(test)]
mod tests;
