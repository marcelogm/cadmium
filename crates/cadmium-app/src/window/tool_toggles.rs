use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use super::CadmiumWindow;
use crate::LOG_DOMAIN;
use crate::tools::keys::{KEYPAD_TOOL_KEYS, TOOL_KEYS, is_number_row_digit, pressed_tool};
use crate::tools::{UiTool, tool_toggle};

impl CadmiumWindow {
    pub(super) fn fill_tool_group(&self) {
        let tool_group = &self.imp().tool_group;
        for tool in UiTool::ALL {
            tool_group.add(tool_toggle(tool));
        }
        tool_group.set_active_name(Some(UiTool::default().name()));
    }

    pub(super) fn change_tool_state(
        &self,
        action: &gio::SimpleAction,
        value: Option<&glib::Variant>,
    ) {
        if let Some(name) = value.and_then(|value| value.str()) {
            self.select_tool(action, name);
        }
    }

    fn select_tool(&self, action: &gio::SimpleAction, name: &str) {
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
        // The number row is the window's digit keys' (`setup_digit_keys`).
        let keys = TOOL_KEYS
            .into_iter()
            .filter(|&(key, _)| !is_number_row_digit(key))
            .chain(KEYPAD_TOOL_KEYS);
        for (key, tool) in keys {
            let shortcut = gtk::Shortcut::builder()
                .trigger(&gtk::KeyvalTrigger::new(key, gdk::ModifierType::empty()))
                .action(&gtk::NamedAction::new("win.tool"))
                .arguments(&tool.name().to_variant())
                .build();
            controller.add_shortcut(shortcut);
        }
        self.add_controller(controller);
    }

    /// Number-row keys pick tools by the physical key: AZERTY's unshifted `&` is its 1,
    /// which no keyval trigger gets right, so the key is read on both levels. Keypad keys and
    /// letters go through the shortcut triggers instead.
    pub(super) fn setup_digit_keys(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |keys, _, keycode, modifiers| window.digit_key_pressed(keys, keycode, modifiers)
        ));
        self.add_controller(keys);
    }

    fn digit_key_pressed(
        &self,
        keys: &gtk::EventControllerKey,
        keycode: u32,
        modifiers: gdk::ModifierType,
    ) -> glib::Propagation {
        if !self.window_keys_are_free() {
            return glib::Propagation::Proceed;
        }
        match pressed_tool(keys, keycode, modifiers) {
            None => glib::Propagation::Proceed,
            Some(tool) => {
                self.pick_painting_tool(tool);
                glib::Propagation::Stop
            }
        }
    }

    fn pick_painting_tool(&self, tool: UiTool) {
        let name = tool.name().to_variant();
        if let Err(error) = WidgetExt::activate_action(self, "win.tool", Some(&name)) {
            glib::g_warning!(LOG_DOMAIN, "couldn't pick {tool:?}: {error}");
        }
    }
}
