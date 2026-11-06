//! The editor's signals: Save, the ⋯ menu, the Brush group, the presets and the sliders.

use std::rc::Rc;

use adw::prelude::*;
use cadmium_studio::{HairPreset, HeadPreset};
use gtk::gio;

use super::brush_group::changes_size;
use super::controls::preset_at;
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

    /// The ⋯ menu's actions; Delete only for a user brush.
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
        self.schematic.queue_draw();
    }

    fn resize(&self, size_px: f64) {
        let current = self.draft.borrow().current().size_px;
        if changes_size(size_px, current) {
            self.draft.borrow_mut().set_size(size_px as f32);
            self.schematic.queue_draw();
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
        let shown = row.clone();
        row.slider.scale.connect_value_changed(move |scale| {
            let value = scale.value();
            with_editor(&editor, |editor| editor.slide(&shown, value));
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

    /// Moves the row's control while its part is Custom, and puts the thumb on the value it
    /// took (on the step grid). `value-changed` doesn't recurse: moving the thumb here
    /// re-emits it once this handler returns, with the snapped value, which then changes
    /// nothing; a thumb already on the grid isn't moved, so it isn't re-emitted at all.
    fn slide(&self, row: &ControlRow, value: f64) {
        if !self.draft.borrow_mut().set(row.control, value as f32) {
            return;
        }
        row.show(row.control.get(self.draft.borrow().current()));
        self.schematic.queue_draw();
    }
}

fn menu_action(editor: &Rc<Editor>, name: &str, run: Run) -> gio::SimpleAction {
    let action = gio::SimpleAction::new(name, None);
    let editor = Rc::downgrade(editor);
    action.connect_activate(move |_, _| with_editor(&editor, run));
    action
}
