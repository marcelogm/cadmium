//! Save, Save as Copy, Export… and Delete: each ends the editor without asking (C1, C2).

use std::rc::{Rc, Weak};

use gtk::glib;

use super::Editor;
use super::closing::Closing;
use super::draft::{Draft, SavePlan};
use crate::LOG_DOMAIN;
use crate::brush_library::{LibraryError, Saved};
use crate::dialogs::{delete_brush_alert, deletion_confirmed};
use crate::state::AppState;
use crate::window::CadmiumWindow;

const SAVE_FAILED: &str = "Can't save brush";
const UNWRITTEN: &str = "; couldn't write it to your brush folder, the change lasts until you quit";

impl Editor {
    pub(super) fn save(self: &Rc<Self>) {
        let stored = self.store_with(store);
        self.after_storing(stored);
    }

    pub(super) fn save_copy(self: &Rc<Self>) {
        let stored = self.store_with(store_copy);
        self.after_storing(stored);
    }

    /// Exports the draft as it would be saved; the editor stays open.
    pub(super) fn export(self: &Rc<Self>) {
        let brush = self.draft.borrow().saved_brush();
        if let Some(window) = self.window.upgrade() {
            window.export_brush_spec(brush);
        }
    }

    /// Asks, then deletes the user brush; the default brush is worn if it was worn (C2).
    pub(super) fn delete(self: &Rc<Self>) {
        let Some(index) = self.draft.borrow().deletable() else {
            return;
        };
        let name = self.draft.borrow().original().name.clone();
        glib::spawn_future_local(delete_confirmed(Rc::downgrade(self), name, index));
    }

    /// Runs `store` on the window's state with the draft; `None` once the window is gone.
    fn store_with(
        &self,
        store: impl FnOnce(&mut AppState, &Draft) -> Result<Saved, LibraryError>,
    ) -> Option<Result<Saved, LibraryError>> {
        let window = self.window.upgrade()?;
        let state = window.state();
        let draft = self.draft.borrow();
        let mut state = state.borrow_mut();
        Some(store(&mut state, &draft))
    }

    /// Closes the editor, then reports how the store went; the rack refreshes on close (C1).
    fn after_storing(self: &Rc<Self>, stored: Option<Result<Saved, LibraryError>>) {
        self.finish(Closing::Done);
        let (Some(window), Some(stored)) = (self.window.upgrade(), stored) else {
            return;
        };
        match stored {
            Ok(saved) => report_saved(&window, &saved),
            Err(error) => window.report_brush_error(SAVE_FAILED, &error),
        }
    }
}

/// Logs a brush that couldn't be written, then refreshes the panels with the saved toast.
fn report_saved(window: &CadmiumWindow, saved: &Saved) {
    if let Some(error) = &saved.save_error {
        glib::g_warning!(LOG_DOMAIN, "couldn't write brush {}: {error}", saved.name);
    }
    window.after_library_change(&saved_toast(saved));
}

/// Save (C3, C8): over a user brush, else as a new brush; either way it is worn after (C1).
fn store(state: &mut AppState, draft: &Draft) -> Result<Saved, LibraryError> {
    match draft.save_plan() {
        SavePlan::Replace(index) => {
            state.save_library_brush(index, draft.original(), &draft.saved_brush())
        }
        SavePlan::AddNew(brush) => Ok(state.add_library_brush(brush)),
    }
}

/// Save as Copy: the draft as a new brush, "<name> (copy)" numbered when taken; worn after (C1).
fn store_copy(state: &mut AppState, draft: &Draft) -> Result<Saved, LibraryError> {
    Ok(state.add_library_brush(draft.copy()))
}

/// "Saved Name", or "Saved as Name (2)" when the name had to change (C8); and when the file
/// couldn't be written, that the brush only lasts for the session.
fn saved_toast(saved: &Saved) -> String {
    let mut toast = if saved.renamed {
        format!("Saved as {}", saved.name)
    } else {
        format!("Saved {}", saved.name)
    };
    if saved.save_error.is_some() {
        toast.push_str(UNWRITTEN);
    }
    toast
}

async fn delete_confirmed(editor: Weak<Editor>, name: String, index: usize) {
    let Some((dialog, alert)) = editor.upgrade().map(|editor| {
        let alert = delete_brush_alert(&name);
        editor.keep_alert(&alert);
        (editor.dialog.clone(), alert)
    }) else {
        return;
    };
    if !deletion_confirmed(alert, &dialog).await {
        return;
    }
    let Some(editor) = editor.upgrade() else {
        return;
    };
    editor.finish(Closing::Done);
    if let Some(window) = editor.window.upgrade() {
        window.delete_library_brush(&name, index);
    }
}

#[cfg(test)]
mod tests;
