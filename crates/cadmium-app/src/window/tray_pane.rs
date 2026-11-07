use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene};

use super::CadmiumWindow;
use super::tray_divider::{TrayDivider, bar_drag_opens, clamp_open_position};
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
        self.set_tray_open(!self.imp().tray_pane.is_visible());
    }

    /// Opens or closes the tray, and remembers which.
    fn set_tray_open(&self, open: bool) {
        if self.imp().tray_pane.is_visible() == open {
            return;
        }
        self.show_tray_pane(open);
        self.remember_tray(|settings| settings.tray_shown = open);
    }

    pub fn show_tray(&self) {
        self.show_tray_pane(true);
    }

    fn show_tray_pane(&self, shown: bool) {
        let imp = self.imp();
        let showing_now = shown && !imp.tray_pane.is_visible();
        imp.tray_pane.set_visible(shown);
        imp.tray_bar.set_visible(!shown);
        if showing_now {
            let mut divider = imp.tray_divider.get();
            divider.forget_layout();
            imp.tray_divider.set(divider);
            self.queue_divider_placing();
        }
    }

    /// Shows the tray as the settings left it, and keeps the divider at the tray's share
    /// of the paned as the window resizes.
    pub(super) fn restore_tray_layout(&self) {
        let settings = self.state().borrow().settings.get();
        let imp = self.imp();
        imp.tray_divider.set(TrayDivider::new(settings.tray_share));
        self.show_tray_pane(settings.tray_shown);
        imp.tray_paned.connect_max_position_notify(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.queue_divider_placing()
        ));
        imp.tray_paned.connect_position_notify(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.follow_divider()
        ));
        self.close_tray_when_let_go_low();
        self.open_tray_from_its_bar();
    }

    /// Closes the tray when the painter drags the divider's handle near the bottom and lets
    /// go. GtkPaned keeps its own drag to itself, so this watches, in the capture phase,
    /// every press and release under the paned (the canvas's and the tray's too): a press
    /// on the handle grabs it, and only that press's release can close the tray.
    fn close_tray_when_let_go_low(&self) {
        let presses = gtk::EventControllerLegacy::new();
        presses.set_propagation_phase(gtk::PropagationPhase::Capture);
        presses.connect_event(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, event| {
                window.watch_divider_press(event);
                glib::Propagation::Proceed
            }
        ));
        self.imp().tray_paned.add_controller(presses);
    }

    fn watch_divider_press(&self, event: &gdk::Event) {
        let imp = self.imp();
        let mut divider = imp.tray_divider.get();
        let closes = match event.event_type() {
            gdk::EventType::ButtonPress | gdk::EventType::TouchBegin
                if self.press_is_on_divider_handle(event) =>
            {
                divider.grab();
                false
            }
            gdk::EventType::ButtonRelease | gdk::EventType::TouchEnd => divider.released(),
            gdk::EventType::TouchCancel => {
                divider.cancel();
                false
            }
            _ => return,
        };
        imp.tray_divider.set(divider);
        if closes {
            self.set_tray_open(false);
        }
    }

    fn press_is_on_divider_handle(&self, event: &gdk::Event) -> bool {
        let paned = &*self.imp().tray_paned;
        let (Some((x, y)), Some(native), Some(handle)) =
            (event.position(), paned.native(), divider_handle(paned))
        else {
            return false;
        };
        let (dx, dy) = native.surface_transform();
        let on_surface = graphene::Point::new((x - dx) as f32, (y - dy) as f32);
        let point = native
            .upcast_ref::<gtk::Widget>()
            .compute_point(paned, &on_surface);
        let bounds = handle.compute_bounds(paned);
        point
            .zip(bounds)
            .is_some_and(|(point, bounds)| bounds.contains_point(&point))
    }

    /// A click on the closed tray's bar opens it, and so does a short drag up.
    fn open_tray_from_its_bar(&self) {
        let bar = &self.imp().tray_bar;
        bar.connect_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.set_tray_open(true)
        ));
        let drag = gtk::GestureDrag::new();
        drag.connect_drag_update(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |drag, _, offset_y| {
                if !window.imp().tray_pane.is_visible() && bar_drag_opens(offset_y) {
                    drag.set_state(gtk::EventSequenceState::Claimed);
                    window.set_tray_open(true);
                }
            }
        ));
        bar.add_controller(drag);
    }

    /// Places the divider once the paned's new height is laid out, outside its allocation.
    fn queue_divider_placing(&self) {
        if self.imp().divider_queued.replace(true) {
            return;
        }
        glib::idle_add_local_full(
            glib::Priority::HIGH_IDLE,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    window.imp().divider_queued.set(false);
                    window.place_divider();
                    glib::ControlFlow::Break
                }
            ),
        );
    }

    fn place_divider(&self) {
        let imp = self.imp();
        let paned = &imp.tray_paned;
        let mut divider = imp.tray_divider.get();
        let Some(position) = divider.place(paned.height()) else {
            return;
        };
        imp.tray_divider.set(divider);
        // Clamped here so GTK has nothing left to clamp, which would look like a drag; the
        // paned lets the tray shrink (to drag it closed), so its minimum is kept here.
        let handle_px = divider_handle(paned).map_or(0, |handle| handle.height());
        let tray_min_px = imp.tray_pane.measure(gtk::Orientation::Vertical, -1).0;
        let position = clamp_open_position(
            position.min(paned.max_position().max(0)),
            paned.min_position(),
            paned.height(),
            handle_px,
            tray_min_px,
        );
        paned.set_position(position);
        let mut divider = imp.tray_divider.get();
        divider.end_place();
        imp.tray_divider.set(divider);
    }

    /// Takes a divider move at the laid-out height as the painter's, and keeps its share.
    fn follow_divider(&self) {
        let imp = self.imp();
        if !imp.tray_pane.is_visible() {
            return;
        }
        let mut divider = imp.tray_divider.get();
        divider.follow_move(imp.tray_paned.position(), imp.tray_paned.height());
        imp.tray_divider.set(divider);
    }

    /// Saves the tray's share; the window calls it as it closes.
    pub(super) fn remember_tray_share(&self) {
        let share = self.imp().tray_divider.get().share();
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

/// The paned's handle, the separator between its children.
fn divider_handle(paned: &gtk::Paned) -> Option<gtk::Widget> {
    let mut child = paned.first_child();
    while let Some(widget) = child {
        if widget.css_name() == "separator" {
            return Some(widget);
        }
        child = widget.next_sibling();
    }
    None
}
