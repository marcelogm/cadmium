use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib};

use super::CadmiumWindow;
use crate::input::nav_drag::{SpacePress, modifiers_after, space_press};

impl CadmiumWindow {
    /// Tracks the keys that ready a navigation drag on the canvas: Space moves, Ctrl zooms,
    /// Shift rotates. Space is taken before the focused widget sees it, even from open
    /// popovers and menus, except by text fields and dialogs.
    pub(super) fn setup_navigation_keys(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| window.key_pressed(key, modifiers)
        ));
        keys.connect_key_released(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, key, _, modifiers| window.key_released(key, modifiers)
        ));
        self.add_controller(keys);
        self.connect_is_active_notify(Self::release_held_keys_when_inactive);
    }

    /// Drops the held keys when the window loses focus, since their releases would go unseen.
    fn release_held_keys_when_inactive(&self) {
        if !self.is_active() {
            self.imp().space_claimed.set(false);
            self.show_held_keys(false, gdk::ModifierType::empty());
        }
    }

    /// Handles a key press: Space starts or keeps a hold; Ctrl and Shift ready zoom or
    /// rotate. Only Space is ever taken; Ctrl and Shift go on to shortcuts and widgets.
    fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        if key == gdk::Key::space {
            return self.space_pressed(modifiers);
        }
        self.show_held_keys(self.space_held(), modifiers_after(key, true, modifiers));
        glib::Propagation::Proceed
    }

    /// Handles a key release: Space ends the hold; Ctrl and Shift let go of zoom or rotate.
    fn key_released(&self, key: gdk::Key, modifiers: gdk::ModifierType) {
        if key == gdk::Key::space {
            self.imp().space_claimed.set(false);
            self.show_held_keys(false, modifiers);
        } else {
            self.show_held_keys(self.space_held(), modifiers_after(key, false, modifiers));
        }
    }

    /// Handles a Space press: claims it while it navigates, and keeps claiming its
    /// auto-repeats until the key is released.
    fn space_pressed(&self, modifiers: gdk::ModifierType) -> glib::Propagation {
        let claimed = self.imp().space_claimed.get();
        match space_press(claimed, self.window_keys_are_free()) {
            SpacePress::Pass => return glib::Propagation::Proceed,
            SpacePress::Hold => self.show_held_keys(true, modifiers),
            SpacePress::Release => self.show_held_keys(false, gdk::ModifierType::empty()),
        }
        self.imp().space_claimed.set(true);
        glib::Propagation::Stop
    }

    /// Whether a Space hold is active on the canvas.
    fn space_held(&self) -> bool {
        self.imp()
            .canvas
            .get()
            .is_some_and(|canvas| canvas.space_held())
    }

    /// Tells the canvas which navigation keys are held; while a dialog is up, none are.
    fn show_held_keys(&self, space_held: bool, modifiers: gdk::ModifierType) {
        let Some(canvas) = self.imp().canvas.get() else {
            return;
        };
        if self.visible_dialog().is_some() {
            canvas.set_held_keys(false, gdk::ModifierType::empty());
        } else {
            canvas.set_held_keys(space_held, modifiers);
        }
    }

    /// Whether the window may take keys for itself (Space for navigation, Shift+digit for
    /// the tray): no dialog is up and no text field has focus.
    pub(super) fn window_keys_are_free(&self) -> bool {
        let typing = gtk::prelude::RootExt::focus(self)
            .is_some_and(|focus| focus.dynamic_cast_ref::<gtk::Editable>().is_some());
        self.visible_dialog().is_none() && !typing
    }
}
