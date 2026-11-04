use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use super::CadmiumWindow;
use crate::LOG_DOMAIN;
use crate::tools::UiTool;
use crate::tools::keys::{KEYPAD_TOOL_KEYS, TOOL_KEYS};

impl CadmiumWindow {
    pub(super) fn change_tool_state(
        &self,
        action: &gio::SimpleAction,
        value: Option<&glib::Variant>,
    ) {
        if let Some(name) = value.and_then(|value| value.str()) {
            self.select_tool(action, name);
        }
    }

    pub(super) fn select_tool(&self, action: &gio::SimpleAction, name: &str) {
        let Some(tool) = UiTool::from_name(name) else {
            glib::g_warning!(LOG_DOMAIN, "unknown tool {name:?}");
            return;
        };
        action.set_state(&name.to_variant());
        self.imp().tool.set(tool);
        if let Some(canvas) = self.imp().canvas.get() {
            canvas.state().borrow_mut().set_ui_tool(tool);
            self.refresh_panels();
        }
        let tool_group = &self.imp().tool_group;
        if tool_group.active_name().as_deref() != Some(name) {
            tool_group.set_active_name(Some(name));
        }
    }

    pub(super) fn follow_tool_group(&self) {
        self.imp()
            .tool_group
            .connect_active_name_notify(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |tool_group| window.follow_active_toggle(tool_group)
            ));
    }

    fn follow_active_toggle(&self, tool_group: &adw::ToggleGroup) {
        let Some(name) = tool_group.active_name() else {
            return;
        };
        if self.imp().tool.get().name() != name {
            self.change_action_state("tool", &name.to_variant());
        }
    }

    pub(super) fn setup_tool_shortcuts(&self) {
        let controller = gtk::ShortcutController::new();
        controller.set_scope(gtk::ShortcutScope::Global);
        controller.set_propagation_phase(gtk::PropagationPhase::Bubble);
        for (key, tool) in TOOL_KEYS.into_iter().chain(KEYPAD_TOOL_KEYS) {
            let shortcut = gtk::Shortcut::builder()
                .trigger(&gtk::KeyvalTrigger::new(key, gdk::ModifierType::empty()))
                .action(&gtk::NamedAction::new("win.tool"))
                .arguments(&tool.name().to_variant())
                .build();
            controller.add_shortcut(shortcut);
        }
        self.add_controller(controller);
    }
}
