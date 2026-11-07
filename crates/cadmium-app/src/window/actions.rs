use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use super::CadmiumWindow;
use crate::brush_editor;
use crate::state::DryOutcome;
use crate::tools::UiTool;

const DRIED_TOAST: &str = "Canvas dried";
const UNDO_LABEL: &str = "Undo";

type Activate = fn(&CadmiumWindow);

const ACTIONS: &[(&str, Activate)] = &[
    ("toggle-sidebar", CadmiumWindow::toggle_sidebar),
    ("toggle-tray", CadmiumWindow::toggle_tray),
    ("new", CadmiumWindow::open_new_painting_dialog),
    ("clean-brush", CadmiumWindow::clean_brush),
    ("undo", CadmiumWindow::undo),
    ("redo", CadmiumWindow::redo),
    ("dry", CadmiumWindow::dry),
    ("clear-tray", CadmiumWindow::clear_tray),
    ("zoom-in", CadmiumWindow::zoom_in),
    ("zoom-out", CadmiumWindow::zoom_out),
    ("zoom-fit", CadmiumWindow::zoom_fit),
    ("rotate-left", CadmiumWindow::rotate_left),
    ("rotate-right", CadmiumWindow::rotate_right),
    ("preferences", CadmiumWindow::open_preferences),
    ("open", CadmiumWindow::open),
    ("save", CadmiumWindow::save),
    ("save-as", CadmiumWindow::save_as),
    ("export", CadmiumWindow::export),
    ("new-brush", CadmiumWindow::new_brush),
    ("edit-brush", CadmiumWindow::edit_brush),
    ("import-brush", CadmiumWindow::import_brush),
    ("export-brush", CadmiumWindow::export_brush),
    ("delete-brush", CadmiumWindow::delete_brush),
];

impl CadmiumWindow {
    pub(super) fn setup_actions(&self) {
        let simple = ACTIONS
            .iter()
            .map(|&(name, activate)| simple_action(name, activate));
        self.add_action_entries(std::iter::once(tool_action()).chain(simple));
        self.set_brush_deletable(false);
    }

    pub(super) fn sync_history_actions(&self) {
        let state = self.state();
        let Ok(state) = state.try_borrow() else {
            return;
        };
        let (can_undo, can_redo) = (state.can_undo(), state.can_redo());
        drop(state);
        for (name, enabled) in [("undo", can_undo), ("redo", can_redo)] {
            self.enable_action(name, enabled);
        }
    }

    pub fn set_brush_deletable(&self, deletable: bool) {
        self.enable_action("delete-brush", deletable);
    }

    fn enable_action(&self, name: &str, enabled: bool) {
        if let Some(action) = self.lookup_action(name).and_downcast::<gio::SimpleAction>()
            && action.is_enabled() != enabled
        {
            action.set_enabled(enabled);
        }
    }

    fn toggle_sidebar(&self) {
        let split_view = &self.imp().split_view;
        split_view.set_show_sidebar(!split_view.shows_sidebar());
    }

    fn clean_brush(&self) {
        self.state().borrow_mut().clean_brush();
        self.refresh_panels();
    }

    fn undo(&self) {
        self.state().borrow_mut().undo();
        self.after_history_change();
    }

    fn redo(&self) {
        self.state().borrow_mut().redo();
        self.after_history_change();
    }

    fn clear_tray(&self) {
        self.state().borrow_mut().clear_tray();
        self.after_contact();
    }

    fn new_brush(&self) {
        brush_editor::open_new(self);
    }

    fn edit_brush(&self) {
        brush_editor::open(self);
    }

    fn zoom_in(&self) {
        self.canvas().zoom_in();
    }

    fn zoom_out(&self) {
        self.canvas().zoom_out();
    }

    fn zoom_fit(&self) {
        self.canvas().zoom_fit();
    }

    fn rotate_left(&self) {
        self.canvas().rotate_step(false);
    }

    fn rotate_right(&self) {
        self.canvas().rotate_step(true);
    }

    fn open_preferences(&self) {
        if self.visible_dialog().is_none() {
            crate::preferences_dialog::preferences_dialog(&self.state()).present(Some(self));
        }
    }

    fn after_history_change(&self) {
        self.canvas().queue_present();
        self.sync_history_actions();
    }

    fn dry(&self) {
        let outcome = self.state().borrow_mut().dry();
        let DryOutcome::Dried { undoable } = outcome else {
            return;
        };
        self.after_history_change();
        let toast = adw::Toast::new(DRIED_TOAST);
        if undoable {
            toast.set_button_label(Some(UNDO_LABEL));
            toast.set_action_name(Some("win.undo"));
        }
        self.imp().toast_overlay.add_toast(toast);
    }
}

fn tool_action() -> gio::ActionEntry<CadmiumWindow> {
    gio::ActionEntry::builder("tool")
        .parameter_type(Some(glib::VariantTy::STRING))
        .state(UiTool::default().name().to_variant())
        .change_state(CadmiumWindow::change_tool_state)
        .build()
}

fn simple_action(name: &str, activate: Activate) -> gio::ActionEntry<CadmiumWindow> {
    gio::ActionEntry::builder(name)
        .activate(move |window: &CadmiumWindow, _, _| activate(window))
        .build()
}
