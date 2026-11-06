//! Ending the editor: Cancel and Esc ask before dropping changes; every other ending (saved,
//! copied, deleted, an error, a lost GPU through the window) closes without asking.

use std::rc::{Rc, Weak};

use adw::prelude::*;

use super::draft::Closing;
use super::{Editor, with_editor};
use crate::dialogs::confirm_discard_brush_changes;

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
            self.dialog.force_close();
            return;
        }
        let dialog = self.dialog.clone();
        let alert = confirm_discard_brush_changes(&self.dialog, move || dialog.force_close());
        self.keep_alert(&alert);
    }

    /// Remembers the alert asking about this editor until it closes, so that closing the
    /// editor (a lost GPU closes it from the window) closes the alert too.
    pub(super) fn keep_alert(self: &Rc<Self>, alert: &adw::AlertDialog) {
        self.alert.replace(Some(alert.clone()));
        let editor = Rc::downgrade(self);
        alert.connect_closed(move |_| forget_alert(&editor));
    }

    /// Closes the alert still asking about this editor, if any.
    pub(super) fn dismiss_alert(&self) {
        let alert = self.alert.take();
        if let Some(alert) = alert {
            alert.force_close();
        }
    }
}

/// The alert closed: the editor, if still open, no longer has one asking about it.
fn forget_alert(editor: &Weak<Editor>) {
    if let Some(editor) = editor.upgrade() {
        editor.alert.take();
    }
}
