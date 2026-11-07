use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib};

use super::CadmiumWindow;
use crate::LOG_DOMAIN;
use crate::settings::Settings;
use crate::tools::keys::{DigitPick, digit_pick};
use crate::tools::{TrayTool, UiTool};

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
                    let Some(name) = group.active_name() else {
                        return;
                    };
                    if window.imp().tray_tool.get().name() != name {
                        window.select_tray_tool(&name);
                    }
                }
            ));
    }

    pub(crate) fn select_tray_tool(&self, name: &str) {
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
        toggle.set_tooltip(&load.tooltip_for(painting));
        toggle.set_icon_name(Some(load.icon_for(painting)));
        toggle.set_enabled(load.is_enabled(painting));
    }

    /// Number-row keys pick tools by the physical key: a digit picks the painting's tool,
    /// Shift and a digit the tray's. Shift+1 arrives as `!` on US and AZERTY's unshifted
    /// `&` is its 1, which no keyval trigger gets right on both, so the key is read on both
    /// levels. Taken in the capture phase so the digit triggers never see these presses;
    /// keypad keys and letters still go through them.
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
        let display = WidgetExt::display(self);
        let level = |state| {
            display
                .translate_key(keycode, state, keys.group() as i32)
                .map(|(key, ..)| key)
        };
        let pick = digit_pick(
            level(gdk::ModifierType::empty()),
            level(gdk::ModifierType::SHIFT_MASK),
            modifiers,
        );
        match pick {
            None => return glib::Propagation::Proceed,
            Some(DigitPick::Painting(tool)) => self.pick_painting_tool(tool),
            // A Load that can't load is not picked, but the press is still the tray's.
            Some(DigitPick::Tray(tool)) if !tool.is_enabled(self.tool()) => {}
            Some(DigitPick::Tray(tool)) => self.select_tray_tool(tool.name()),
        }
        glib::Propagation::Stop
    }

    fn pick_painting_tool(&self, tool: UiTool) {
        let name = tool.name().to_variant();
        if let Err(error) = WidgetExt::activate_action(self, "win.tool", Some(&name)) {
            glib::g_warning!(LOG_DOMAIN, "couldn't pick {tool:?}: {error}");
        }
    }

    pub(super) fn toggle_tray(&self) {
        let sheet = &self.imp().tray_sheet;
        sheet.set_open(!sheet.is_open());
    }

    pub fn show_tray(&self) {
        self.imp().tray_sheet.set_open(true);
    }

    /// Opens the tray's sheet if the settings left it open, and remembers each opening and
    /// closing after that.
    pub(super) fn restore_tray_sheet(&self) {
        let shown = self.state().borrow().settings.get().tray_shown;
        let sheet = &self.imp().tray_sheet;
        sheet.set_open(shown);
        sheet.connect_open_notify(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |sheet| {
                let open = sheet.is_open();
                window.remember_tray(|settings| settings.tray_shown = open);
            }
        ));
        self.keep_zoom_osd_above_the_tray();
    }

    /// The sheet and its bar lie over the canvas, which keeps its size (resizing it as the
    /// sheet slides would repaint it every frame); only the zoom OSD moves up out of their way.
    fn keep_zoom_osd_above_the_tray(&self) {
        let sheet = &self.imp().tray_sheet;
        let lift = glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |sheet: &adw::BottomSheet| {
                let margin = zoom_osd_margin(sheet.bottom_bar_height(), sheet.sheet_height());
                window.imp().zoom_osd.set_margin_bottom(margin);
                if let Some(canvas) = window.imp().canvas.get() {
                    canvas.set_fit_inset(sheet.bottom_bar_height());
                }
            }
        );
        sheet.connect_bottom_bar_height_notify(lift.clone());
        sheet.connect_sheet_height_notify(lift);
    }

    fn remember_tray(&self, change: impl FnOnce(&mut Settings)) {
        let Some(canvas) = self.imp().canvas.get() else {
            return;
        };
        if let Err(error) = canvas.state().borrow_mut().settings.update(change) {
            glib::g_warning!(
                LOG_DOMAIN,
                "couldn't save whether the tray is open: {error}"
            );
        }
    }
}

/// The zoom OSD's margin from the canvas's edges.
const OSD_MARGIN: i32 = 12;

/// The zoom OSD's bottom margin with the tray's bar `bottom_bar_height` and its sheet
/// `sheet_height` tall over the canvas.
fn zoom_osd_margin(bottom_bar_height: i32, sheet_height: i32) -> i32 {
    OSD_MARGIN + bottom_bar_height.max(sheet_height)
}

#[cfg(test)]
mod tests;
