//! The editor's signals: Save, the ⋯ menu, the Brush group, the presets and the sliders.

use std::rc::Rc;

use adw::prelude::*;
use cadmium_studio::{HairPreset, HeadPreset};
use gtk::gio;

use super::brush_group::changes_size;
use super::controls::{Control, preset_at};
use super::header::{ACTION_GROUP, Header};
use super::parts::ControlRow;
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
        self.connect_parts();
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

    fn connect_parts(self: &Rc<Self>) {
        let editor = Rc::downgrade(self);
        self.parts
            .head_preset
            .connect_selected_notify(move |dropdown| {
                let position = dropdown.selected();
                with_editor(&editor, |editor| editor.pick_head(position));
            });
        let editor = Rc::downgrade(self);
        self.parts
            .hair_preset
            .connect_selected_notify(move |dropdown| {
                let position = dropdown.selected();
                with_editor(&editor, |editor| editor.pick_hair(position));
            });
        for row in &self.parts.rows {
            self.connect_control(row);
        }
    }

    fn connect_control(self: &Rc<Self>, row: &ControlRow) {
        let editor = Rc::downgrade(self);
        let control = row.control;
        let label = row.slider.value_label.clone();
        row.slider.scale.connect_value_changed(move |scale| {
            let value = scale.value();
            with_editor(&editor, |editor| editor.slide(control, value, &label));
        });
    }

    fn pick_head(&self, position: u32) {
        let preset = preset_at(&HeadPreset::ALL, position);
        self.draft.borrow_mut().pick_head(preset);
        self.sync();
    }

    fn pick_hair(&self, position: u32) {
        let preset = preset_at(&HairPreset::ALL, position);
        self.draft.borrow_mut().pick_hair(preset);
        self.sync();
    }

    /// Moves `control` while its part is Custom, and shows the value it took.
    fn slide(&self, control: Control, value: f64, label: &gtk::Label) {
        if !self.draft.borrow_mut().set(control, value as f32) {
            return;
        }
        let shown = control.get(self.draft.borrow().current());
        label.set_label(&control.format(shown));
    }
}

fn menu_action(editor: &Rc<Editor>, name: &str, run: Run) -> gio::SimpleAction {
    let action = gio::SimpleAction::new(name, None);
    let editor = Rc::downgrade(editor);
    action.connect_activate(move |_, _| with_editor(&editor, run));
    action
}
