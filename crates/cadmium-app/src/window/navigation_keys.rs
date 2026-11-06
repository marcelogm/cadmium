use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib};

use super::CadmiumWindow;
use crate::input::nav_drag::{SpacePress, modifiers_after, space_mode, space_press};

impl CadmiumWindow {
    /// Holding Space readies a navigation drag on the canvas (Ctrl: zoom, Shift: rotate).
    /// Space is taken before the focused widget sees it, even from open popovers and menus,
    /// except by text fields and dialogs.
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
        self.connect_is_active_notify(Self::release_space_when_inactive);
    }

    /// Drops the Space hold when the window loses focus, since the release would go unseen.
    fn release_space_when_inactive(&self) {
        if !self.is_active() {
            self.imp().space_claimed.set(false);
            self.hold_space(None);
        }
    }

    /// Handles a key press: Space starts or keeps a hold; Ctrl and Shift retune a hold.
    fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        if key == gdk::Key::space {
            return self.space_pressed(modifiers);
        }
        self.retune_space(modifiers_after(key, true, modifiers));
        glib::Propagation::Proceed
    }

    /// Handles a key release: Space ends the hold; Ctrl and Shift retune it.
    fn key_released(&self, key: gdk::Key, modifiers: gdk::ModifierType) {
        if key == gdk::Key::space {
            self.imp().space_claimed.set(false);
            self.hold_space(None);
        } else {
            self.retune_space(modifiers_after(key, false, modifiers));
        }
    }

    /// Handles a Space press: claims it while it navigates, and keeps claiming its
    /// auto-repeats until the key is released.
    fn space_pressed(&self, modifiers: gdk::ModifierType) -> glib::Propagation {
        let claimed = self.imp().space_claimed.get();
        match space_press(claimed, self.space_navigates()) {
            SpacePress::Pass => return glib::Propagation::Proceed,
            SpacePress::Hold => self.hold_space(Some(modifiers)),
            SpacePress::Release => self.hold_space(None),
        }
        self.imp().space_claimed.set(true);
        glib::Propagation::Stop
    }

    /// Re-picks the held mode under `modifiers`, if Space is held.
    fn retune_space(&self, modifiers: gdk::ModifierType) {
        if self.space_held() {
            self.hold_space(Some(modifiers));
        }
    }

    /// Whether a Space hold is active on the canvas.
    fn space_held(&self) -> bool {
        self.imp()
            .canvas
            .get()
            .is_some_and(|canvas| canvas.space_mode().is_some())
    }

    /// Readies the canvas's drag mode for `modifiers`, or clears it when `None`.
    fn hold_space(&self, modifiers: Option<gdk::ModifierType>) {
        if let Some(canvas) = self.imp().canvas.get() {
            canvas.set_space_mode(modifiers.map(space_mode));
        }
    }

    /// Whether Space is free for navigation: no dialog is up and no text field has focus.
    fn space_navigates(&self) -> bool {
        let typing = gtk::prelude::RootExt::focus(self)
            .is_some_and(|focus| focus.dynamic_cast_ref::<gtk::Editable>().is_some());
        self.visible_dialog().is_none() && !typing
    }
}
