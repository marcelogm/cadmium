use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib};

use super::CadmiumWindow;
use crate::LOG_DOMAIN;
use crate::settings::{Settings, divider_position, tray_share_at};
use crate::tools::TrayTool;

impl CadmiumWindow {
    pub(super) fn fill_tray_tool_group(&self) {
        let group = &self.imp().tray_tool_group;
        for tool in TrayTool::ALL {
            group.add(
                adw::Toggle::builder()
                    .name(tool.name())
                    .icon_name(tool.icon_for(self.tool()))
                    .tooltip(tool.profile().tooltip)
                    .build(),
            );
        }
        group.set_active_name(Some(TrayTool::default().name()));
    }

    pub(super) fn follow_tray_tool_group(&self) {
        self.imp()
            .tray_tool_group
            .connect_active_name_notify(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |group| {
                    if let Some(name) = group.active_name() {
                        window.select_tray_tool(&name);
                    }
                }
            ));
    }

    fn select_tray_tool(&self, name: &str) {
        let Some(tool) = TrayTool::from_name(name) else {
            glib::g_warning!(LOG_DOMAIN, "unknown tray tool {name:?}");
            return;
        };
        self.imp().tray_tool.set(tool);
        if let Some(canvas) = self.imp().canvas.get() {
            canvas.state().borrow_mut().set_tray_tool(tool);
            self.refresh_panels();
        }
        let group = &self.imp().tray_tool_group;
        if group.active_name().as_deref() != Some(name) {
            group.set_active_name(Some(name));
        }
    }

    /// Names and enables Load after the painting's tool, which is what it loads.
    pub(super) fn sync_load_toggle(&self) {
        let painting = self.tool();
        let load = TrayTool::Load;
        let Some(toggle) = self.imp().tray_tool_group.toggle_by_name(load.name()) else {
            return;
        };
        toggle.set_label(Some(&load.label_for(painting)));
        toggle.set_icon_name(Some(load.icon_for(painting)));
        toggle.set_enabled(load.is_enabled(painting));
    }

    /// Shift+digit picks the tray's tool. Shift+1 arrives as `!` (or whatever the layout
    /// puts there), which no `<Shift>1` trigger matches, so the key is read back unshifted.
    pub(super) fn setup_tray_tool_keys(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |keys, _, keycode, modifiers| window.tray_key_pressed(keys, keycode, modifiers)
        ));
        self.add_controller(keys);
    }

    fn tray_key_pressed(
        &self,
        keys: &gtk::EventControllerKey,
        keycode: u32,
        modifiers: gdk::ModifierType,
    ) -> glib::Propagation {
        let unshifted = WidgetExt::display(self)
            .translate_key(keycode, gdk::ModifierType::empty(), keys.group() as i32)
            .map(|(key, ..)| key);
        match unshifted.and_then(|key| TrayTool::for_shifted_key(key, modifiers)) {
            Some(tool) => {
                self.select_tray_tool(tool.name());
                glib::Propagation::Stop
            }
            None => glib::Propagation::Proceed,
        }
    }

    pub(super) fn toggle_tray(&self) {
        let shown = !self.imp().tray_pane.is_visible();
        self.show_tray_pane(shown);
        self.remember_tray(|settings| settings.tray_shown = shown);
    }

    pub fn show_tray(&self) {
        self.show_tray_pane(true);
    }

    fn show_tray_pane(&self, shown: bool) {
        let imp = self.imp();
        imp.tray_pane.set_visible(shown);
        if imp.tray_toggle.is_active() != shown {
            imp.tray_toggle.set_active(shown);
        }
    }

    /// Lays the divider out from the settings once the paned has a height.
    pub(super) fn restore_tray_layout(&self) {
        let settings = self.state().borrow().settings.get();
        self.show_tray_pane(settings.tray_shown);
        self.imp()
            .tray_paned
            .connect_notify_local(Some("max-position"), move |paned, _| {
                lay_out_divider(paned, settings.tray_share);
            });
    }

    /// Saves where the divider is; the window calls it as it closes.
    pub(super) fn remember_tray_share(&self) {
        let imp = self.imp();
        let height = imp.tray_paned.height();
        if !imp.tray_pane.is_visible() || height <= 0 {
            return;
        }
        let share = tray_share_at(imp.tray_paned.position(), height);
        self.remember_tray(|settings| settings.tray_share = share);
    }

    fn remember_tray(&self, change: impl FnOnce(&mut Settings)) {
        let Some(canvas) = self.imp().canvas.get() else {
            return;
        };
        if let Err(error) = canvas.state().borrow_mut().settings.update(change) {
            glib::g_warning!(LOG_DOMAIN, "couldn't save the tray layout: {error}");
        }
    }
}

/// Puts the divider where `share` leaves the tray, once the paned has a height and until
/// it has a position of its own.
fn lay_out_divider(paned: &gtk::Paned, share: f32) {
    let height = paned.height();
    if height > 0 && !paned.is_position_set() {
        paned.set_position(divider_position(share, height));
    }
}
