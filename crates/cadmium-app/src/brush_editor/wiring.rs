//! The editor's signals: the header's Save, the ⋯ menu, and the Brush group's rows.

use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;

use super::brush_group::changes_size;
use super::header::{ACTION_GROUP, Header};
use super::{Editor, with_editor};

type Run = fn(&Rc<Editor>);

const DELETE: &str = "delete";
/// The ⋯ menu's actions, as `editor.<name>`.
const MENU_ACTIONS: [(&str, Run); 3] = [
    ("save-copy", Editor::save_copy),
    ("export", Editor::export),
    (DELETE, Editor::delete),
];

impl Editor {
    pub(super) fn connect(self: &Rc<Self>, header: &Header) {
        self.connect_closing(&header.cancel);
        let editor = Rc::downgrade(self);
        header
            .save
            .connect_clicked(move |_| with_editor(&editor, Editor::save));
        self.install_menu();
        self.connect_brush_group();
    }

    /// The ⋯ menu's actions; Delete only for a user brush (C2).
    fn install_menu(self: &Rc<Self>) {
        let deletable = self.draft.borrow().deletable().is_some();
        let group = gio::SimpleActionGroup::new();
        for (name, run) in MENU_ACTIONS {
            let action = menu_action(self, name, run);
            action.set_enabled(name != DELETE || deletable);
            group.add_action(&action);
        }
        self.dialog.insert_action_group(ACTION_GROUP, Some(&group));
    }

    fn connect_brush_group(self: &Rc<Self>) {
        let editor = Rc::downgrade(self);
        self.brush.name.connect_changed(move |entry| {
            let name = entry.text();
            with_editor(&editor, |editor| editor.rename(&name));
        });
        let editor = Rc::downgrade(self);
        self.brush.size.connect_value_notify(move |row| {
            let size_px = row.value();
            with_editor(&editor, |editor| editor.resize(size_px));
        });
    }

    fn rename(&self, name: &str) {
        self.draft.borrow_mut().set_name(name);
        self.dialog.set_title(&self.draft.borrow().chosen_name());
    }

    fn resize(&self, size_px: f64) {
        let current = self.draft.borrow().current().size_px;
        if changes_size(size_px, current) {
            self.draft.borrow_mut().set_size(size_px as f32);
        }
    }
}

fn menu_action(editor: &Rc<Editor>, name: &str, run: Run) -> gio::SimpleAction {
    let action = gio::SimpleAction::new(name, None);
    let editor = Rc::downgrade(editor);
    action.connect_activate(move |_, _| with_editor(&editor, run));
    action
}
