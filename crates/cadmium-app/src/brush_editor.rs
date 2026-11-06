//! The brush editor: edits the worn brush as a draft, then saves it to the library.
//!
//! Three columns (D2): the tip's drawing with the example strokes under it, the test sheet,
//! and the controls. Until stages 7b and 7c, the examples and the test sheet are blank paper.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::glib;

use crate::state::AppState;
use crate::window::CadmiumWindow;

mod brush_group;
mod closing;
mod draft;
mod header;
mod layout;
mod listening;
mod paper;
mod saving;
mod wiring;

use brush_group::BrushGroup;
use draft::{Draft, Origin};
use listening::Listening;

/// The open editor. Signal handlers hold it weakly; the dialog's `closed` handler holds the
/// one strong reference, and lets go of it there.
struct Editor {
    window: glib::WeakRef<CadmiumWindow>,
    dialog: adw::Dialog,
    draft: RefCell<Draft>,
    /// Whether the widgets' signals reach the editor: not while it fills them, nor once closing.
    listening: Listening,
    /// The alert asking about the editor ("Discard Changes?", "Delete …?"), while it is open.
    alert: RefCell<Option<adw::AlertDialog>>,
    brush: BrushGroup,
}

/// Shows the editor on the worn brush; while one is open, does nothing.
pub fn open(window: &CadmiumWindow) {
    if window.has_editor() {
        return;
    }
    let draft = draft_for(&window.state().borrow());
    let editor = Editor::new(window, draft);
    window.keep_editor(&editor.dialog);
    editor.dialog.present(Some(window));
}

/// C3: the worn library brush as the library holds it (at its suggested size, whatever size
/// the painting uses now); a worn brush in no library is edited like a factory one.
fn draft_for(state: &AppState) -> Draft {
    let worn = state
        .worn_library_brush()
        .and_then(|index| Some((index, state.brushes.get(index)?.clone())));
    match worn {
        Some((index, brush)) if state.brushes.is_user(index) => {
            Draft::open(brush, Origin::User(index))
        }
        Some((_, brush)) => Draft::open(brush, Origin::Factory),
        None => Draft::open(state.worn_brush_spec(), Origin::Factory),
    }
}

impl Editor {
    fn new(window: &CadmiumWindow, draft: Draft) -> Rc<Editor> {
        let header = header::header();
        let (dialog, columns) = layout::dialog(&draft.chosen_name(), &header.bar);
        let brush = brush_group::brush_group();
        columns.controls.append(&brush.group);
        let editor = Rc::new(Editor {
            window: window.downgrade(),
            dialog,
            draft: RefCell::new(draft),
            listening: Listening::default(),
            alert: RefCell::new(None),
            brush,
        });
        editor.connect(&header);
        editor.sync();
        editor.keep_until_closed();
        editor
    }

    /// Fills every widget from the draft.
    fn sync(&self) {
        self.listening.set_syncing(true);
        self.brush.show(self.draft.borrow().current());
        self.listening.set_syncing(false);
    }

    /// Keeps the editor alive while its dialog is open; once closed, closes an alert still
    /// asking about it and lets the window know.
    fn keep_until_closed(self: &Rc<Self>) {
        let keeper = RefCell::new(Some(Rc::clone(self)));
        let window = self.window.clone();
        self.dialog
            .connect_closed(move |_| after_closed(&keeper, &window));
    }
}

/// The closed editor's last act: closes an alert still asking about it, lets go of it, and
/// tells the window.
fn after_closed(keeper: &RefCell<Option<Rc<Editor>>>, window: &glib::WeakRef<CadmiumWindow>) {
    if let Some(editor) = keeper.take() {
        editor.dismiss_alert();
    }
    if let Some(window) = window.upgrade() {
        window.after_editor_closed();
    }
}

/// Runs `action` on the editor while it is open, not closing and not filling its widgets.
fn with_editor(editor: &Weak<Editor>, action: impl FnOnce(&Rc<Editor>)) {
    if let Some(editor) = editor.upgrade()
        && editor.listening.heeds()
    {
        action(&editor);
    }
}

#[cfg(test)]
mod tests;
