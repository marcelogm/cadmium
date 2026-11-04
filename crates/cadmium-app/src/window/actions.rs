use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use super::CadmiumWindow;
use crate::state::DryOutcome;
use crate::tools::UiTool;

const DRIED_TOAST: &str = "Tela seca";
const UNDO_LABEL: &str = "Desfazer";

impl CadmiumWindow {
    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
    pub(super) fn setup_actions(&self) {
        let tool = gio::ActionEntry::builder("tool")
            .parameter_type(Some(glib::VariantTy::STRING))
            .state(UiTool::default().name().to_variant())
            .change_state(|window: &Self, action, value| {
                if let Some(name) = value.and_then(|value| value.str()) {
                    window.select_tool(action, name);
                }
            })
            .build();
        let toggle_sidebar = gio::ActionEntry::builder("toggle-sidebar")
            .activate(|window: &Self, _, _| {
                let split_view = &window.imp().split_view;
                split_view.set_show_sidebar(!split_view.shows_sidebar());
            })
            .build();
        let new = gio::ActionEntry::builder("new")
            .activate(|window: &Self, _, _| window.open_new_painting_dialog())
            .build();
        let clean_brush = gio::ActionEntry::builder("clean-brush")
            .activate(|window: &Self, _, _| {
                window.state().borrow_mut().clean_brush();
                window.refresh_panels();
            })
            .build();
        let undo = gio::ActionEntry::builder("undo")
            .activate(|window: &Self, _, _| {
                window.state().borrow_mut().undo();
                window.after_history_change();
            })
            .build();
        let redo = gio::ActionEntry::builder("redo")
            .activate(|window: &Self, _, _| {
                window.state().borrow_mut().redo();
                window.after_history_change();
            })
            .build();
        let dry = gio::ActionEntry::builder("dry")
            .activate(|window: &Self, _, _| window.dry())
            .build();
        let clear_tray = gio::ActionEntry::builder("clear-tray")
            .activate(|window: &Self, _, _| {
                window.state().borrow_mut().clear_tray();
                window.after_contact();
            })
            .build();
        let zoom_in = gio::ActionEntry::builder("zoom-in")
            .activate(|window: &Self, _, _| window.canvas().zoom_in())
            .build();
        let zoom_out = gio::ActionEntry::builder("zoom-out")
            .activate(|window: &Self, _, _| window.canvas().zoom_out())
            .build();
        let zoom_fit = gio::ActionEntry::builder("zoom-fit")
            .activate(|window: &Self, _, _| window.canvas().zoom_fit())
            .build();
        let open = gio::ActionEntry::builder("open")
            .activate(|window: &Self, _, _| window.open())
            .build();
        let save = gio::ActionEntry::builder("save")
            .activate(|window: &Self, _, _| window.save())
            .build();
        let save_as = gio::ActionEntry::builder("save-as")
            .activate(|window: &Self, _, _| window.save_as())
            .build();
        let export = gio::ActionEntry::builder("export")
            .activate(|window: &Self, _, _| window.export())
            .build();
        self.add_action_entries([
            tool,
            toggle_sidebar,
            new,
            clean_brush,
            undo,
            redo,
            dry,
            clear_tray,
            zoom_in,
            zoom_out,
            zoom_fit,
            open,
            save,
            save_as,
            export,
        ]);
    }

    #[expect(clippy::excessive_nesting)]
    pub(super) fn sync_history_actions(&self) {
        let state = self.state();
        let Ok(state) = state.try_borrow() else {
            return;
        };
        let (can_undo, can_redo) = (state.can_undo(), state.can_redo());
        drop(state);
        for (name, enabled) in [("undo", can_undo), ("redo", can_redo)] {
            if let Some(action) = self.lookup_action(name).and_downcast::<gio::SimpleAction>()
                && action.is_enabled() != enabled
            {
                action.set_enabled(enabled);
            }
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
