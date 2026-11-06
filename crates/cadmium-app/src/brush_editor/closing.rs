//! Ending the editor: Cancel and Esc ask before dropping changes; every other ending (saved,
//! copied, deleted, an error, a lost GPU through the window) closes without asking.

use std::rc::{Rc, Weak};

use adw::prelude::*;

use super::{Editor, with_editor};
use crate::dialogs::confirm_discard_brush_changes;

/// How the editor ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Closing {
    /// Cancel or Esc: asks before dropping changes.
    Cancel,
    /// Saved, saved as a copy, deleted, or given up after an error: never asks.
    Done,
}

impl Editor {
    pub(super) fn connect_closing(self: &Rc<Self>, cancel: &gtk::Button) {
        let editor = Rc::downgrade(self);
        cancel.connect_clicked(move |_| {
            with_editor(&editor, |editor| editor.finish(Closing::Cancel));
        });
        let editor = Rc::downgrade(self);
        self.dialog.connect_close_attempt(move |_| {
            with_editor(&editor, |editor| editor.finish(Closing::Cancel));
        });
    }

    /// Closes the dialog; a cancel with changes asks "Discard Changes?" first.
    pub(super) fn finish(self: &Rc<Self>, closing: Closing) {
        if !self.draft.borrow().asks_before(closing) {
            self.close_now();
            return;
        }
        let editor = Rc::downgrade(self);
        let alert = confirm_discard_brush_changes(&self.dialog, move || close(&editor));
        self.keep_alert(&alert);
    }

    /// Closes the dialog without asking, once: a closing editor is left to close.
    fn close_now(&self) {
        if self.listening.is_closing() {
            return;
        }
        self.stop();
        self.dialog.force_close();
    }

    /// From here on neither its widgets nor its example painting reach the editor. The
    /// window calls it too, as it closes the editor itself.
    pub(super) fn stop(&self) {
        self.cancel_debounce();
        self.listening.stop();
    }

    /// Remembers the alert asking about this editor until it closes, so that closing the
    /// editor (a lost GPU closes it from the window) closes the alert too.
    pub(super) fn keep_alert(self: &Rc<Self>, alert: &adw::AlertDialog) {
        self.alert.replace(Some(alert.clone()));
        let editor = Rc::downgrade(self);
        alert.connect_closed(move |closed| forget_alert(&editor, closed));
    }

    /// Closes the alert still asking about this editor, if any.
    pub(super) fn dismiss_alert(&self) {
        let alert = self.alert.take();
        if let Some(alert) = alert {
            alert.force_close();
        }
    }
}

/// The changes were discarded: the editor, if still open, closes.
fn close(editor: &Weak<Editor>) {
    if let Some(editor) = editor.upgrade() {
        editor.close_now();
    }
}

/// `closed` closed: the editor, if still open and still keeping that very alert, forgets it.
fn forget_alert(editor: &Weak<Editor>, closed: &adw::AlertDialog) {
    let Some(editor) = editor.upgrade() else {
        return;
    };
    let kept = editor.alert.borrow().as_ref() == Some(closed);
    if kept {
        editor.alert.take();
    }
}
