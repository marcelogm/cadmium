use std::rc::{Rc, Weak};

use adw::prelude::*;
use cadmium_engine::palette::ColourRef;
use gtk::{gdk, gio, glib};

use super::column::{ACTION_GROUP, REMOVE_ACTION};
use super::presentation::{ToolKey, mode_named, tool_key};
use super::{PaletteEditor, with_editor};
use crate::brush_panel::colour_click;
use crate::colour_wheel::PickedPoint;
use crate::state::{AppState, PaletteMode};
use crate::tools::UiTool;
use crate::tools::keys::{digit_pick, plain_key_pick};
use crate::window::CadmiumWindow;

const HISTORY_ACTIONS: [&str; 2] = ["undo", "redo"];

impl PaletteEditor {
    pub(super) fn connect(self: &Rc<Self>, window: &CadmiumWindow) {
        self.connect_modes();
        self.connect_circle();
        self.connect_column();
        self.connect_mixing();
        self.connect_sheet();
        self.connect_keys();
        self.follow_history(window);
    }

    fn connect_modes(self: &Rc<Self>) {
        let editor = Rc::downgrade(self);
        self.modes.connect_active_name_notify(move |modes| {
            let mode = modes.active_name().as_deref().and_then(mode_named);
            changing(&editor, |state| switch_mode(state, mode));
        });
    }

    fn connect_circle(self: &Rc<Self>) {
        let page = &self.circle;
        let editor = Rc::downgrade(self);
        page.wheel
            .connect_picked(move |point| changing(&editor, |state| choose_at(state, point)));
        let editor = Rc::downgrade(self);
        page.wheel.connect_colour_dropped(move |colour, turn| {
            changing(&editor, |state| state.drop_on_circle(colour, turn));
        });
        let editor = Rc::downgrade(self);
        page.wheel
            .connect_rim_dragged(move |from, turn| drag_rim(&editor, from, turn));
        let editor = Rc::downgrade(self);
        page.wheel.connect_rim_removed(move |position| {
            changing(&editor, |state| state.remove_circle_colour(position));
        });
        let editor = Rc::downgrade(self);
        page.darken.connect_value_changed(move |scale| {
            let darken = scale.value() as f32;
            changing(&editor, |state| state.set_palette_darken(darken));
        });
    }

    fn connect_column(self: &Rc<Self>) {
        let column = &self.column;
        let editor = Rc::downgrade(self);
        column
            .pick
            .connect_clicked(move |_| changing(&editor, |state| state.arm_sheet_pick()));
        let editor = Rc::downgrade(self);
        column
            .pin
            .connect_clicked(move |_| changing(&editor, |state| state.pin_chosen()));
        let remove = gio::SimpleAction::new(REMOVE_ACTION, None);
        let editor = Rc::downgrade(self);
        remove.connect_activate(move |_, _| {
            with_editor(&editor, |editor| editor.remove_offered());
        });
        let actions = gio::SimpleActionGroup::new();
        actions.add_action(&remove);
        self.dialog
            .insert_action_group(ACTION_GROUP, Some(&actions));
    }

    fn connect_mixing(self: &Rc<Self>) {
        let page = &self.mixing;
        let editor = Rc::downgrade(self);
        page.tools.connect_active_name_notify(move |tools| {
            let tool = tools.active_name().as_deref().and_then(UiTool::from_name);
            changing(&editor, |state| tool.map(|tool| state.set_sheet_tool(tool)));
        });
        let editor = Rc::downgrade(self);
        page.clear
            .connect_clicked(move |_| changing(&editor, |state| state.clear_sheet()));
    }

    fn connect_sheet(self: &Rc<Self>) {
        let sheet = &self.mixing.sheet;
        let editor = Rc::downgrade(self);
        sheet.connect_released(move |_| with_editor(&editor, |editor| editor.after_change()));
        let editor = Rc::downgrade(self);
        sheet.connect_colour_picked(move |_, _| {
            with_editor(&editor, |editor| editor.after_change());
        });
        let window = self.window.clone();
        sheet.connect_presented(move |_| with_window(&window, CadmiumWindow::sync_history_actions));
        let window = self.window.clone();
        sheet.connect_gpu_lost(move |_| with_window(&window, CadmiumWindow::announce_gpu_lost));
        let window = self.window.clone();
        sheet.connect_presentation_fallback(move |_| {
            with_window(&window, CadmiumWindow::announce_copy_presentation);
        });
    }

    /// Tool keys go to the sheet: its tools are chosen, and the pencil and Move are kept
    /// from the painting's shortcuts underneath.
    fn connect_keys(self: &Rc<Self>) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let editor = Rc::downgrade(self);
        keys.connect_key_pressed(
            move |keys, key, keycode, modifiers| match editor.upgrade() {
                Some(editor) => editor.tool_key_pressed(keys, key, keycode, modifiers),
                None => glib::Propagation::Proceed,
            },
        );
        self.dialog.add_controller(keys);
    }

    fn tool_key_pressed(
        self: &Rc<Self>,
        keys: &gtk::EventControllerKey,
        key: gdk::Key,
        keycode: u32,
        modifiers: gdk::ModifierType,
    ) -> glib::Propagation {
        let display = self.dialog.display();
        let level = |state| {
            display
                .translate_key(keycode, state, keys.group() as i32)
                .map(|(key, ..)| key)
        };
        let picked = digit_pick(
            level(gdk::ModifierType::empty()),
            level(gdk::ModifierType::SHIFT_MASK),
            modifiers,
        )
        .or_else(|| plain_key_pick(key, modifiers));
        match tool_key(picked) {
            ToolKey::NotATool => glib::Propagation::Proceed,
            ToolKey::Swallow => glib::Propagation::Stop,
            ToolKey::Choose(tool) => {
                self.choose_sheet_tool(tool);
                glib::Propagation::Stop
            }
        }
    }

    /// Shows what the window's undo and redo did to the sheet; the handlers go with the dialog.
    fn follow_history(self: &Rc<Self>, window: &CadmiumWindow) {
        let actions = HISTORY_ACTIONS.iter().filter_map(|name| {
            window
                .lookup_action(name)
                .and_downcast::<gio::SimpleAction>()
        });
        let handlers = actions
            .map(|action| {
                let editor = Rc::downgrade(self);
                let handler = action
                    .connect_activate(move |_, _| with_editor(&editor, |editor| editor.refresh()));
                (action, handler)
            })
            .collect();
        self.history_handlers.replace(handlers);
    }

    /// A click on a colour of the column loads the sheet's tool with it, while the sheet shows;
    /// with Ctrl, mixes it in.
    pub(super) fn click_colour(self: &Rc<Self>, colour: ColourRef, modifiers: gdk::ModifierType) {
        if self.state.borrow().palette_edit.mode != PaletteMode::Mixing {
            return;
        }
        let click = colour_click(modifiers);
        self.change(|state| state.load_sheet_colour(colour, click));
    }

    fn remove_offered(self: &Rc<Self>) {
        if let Some(index) = self.column.offered.take() {
            self.change(|state| state.unpin(index));
        }
    }
}

/// Makes `change` to the state through the open editor, which then shows it.
fn changing<T>(editor: &Weak<PaletteEditor>, change: impl FnOnce(&mut AppState) -> T) {
    with_editor(editor, |editor| drop(editor.change(change)));
}

fn switch_mode(state: &mut AppState, mode: Option<PaletteMode>) {
    if let Some(mode) = mode {
        state.set_palette_mode(mode);
    }
}

fn choose_at(state: &mut AppState, point: PickedPoint) {
    state.choose_on_circle(point.turn, point.radius);
}

fn with_window(window: &glib::WeakRef<CadmiumWindow>, action: fn(&CadmiumWindow)) {
    if let Some(window) = window.upgrade() {
        action(&window);
    }
}

/// Moves a grabbed rim dot and answers where it is now; with the dialog gone or filling its
/// widgets, it stays where it was.
fn drag_rim(editor: &Weak<PaletteEditor>, from: usize, turn: f32) -> usize {
    let mut moved = from;
    with_editor(editor, |editor| {
        moved = editor.change(|state| state.drag_circle_colour(from, turn));
    });
    moved
}
