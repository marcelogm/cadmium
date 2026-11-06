use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib};

use super::CadmiumWindow;
use crate::input::nav_drag::space_mode;

impl CadmiumWindow {
    /// Holding Space readies a navigation drag on the canvas (Ctrl: zoom, Shift: rotate).
    /// Space is taken before the focused widget sees it, except by text fields and dialogs.
    pub(super) fn setup_navigation_keys(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| window.space_pressed(key, modifiers)
        ));
        keys.connect_key_released(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, key, _, _| {
                if key == gdk::Key::space {
                    window.hold_space(None);
                }
            }
        ));
        keys.connect_modifiers(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, modifiers| window.space_modifiers(modifiers)
        ));
        self.add_controller(keys);
        self.connect_is_active_notify(Self::release_space_when_inactive);
    }

    fn release_space_when_inactive(&self) {
        if !self.is_active() {
            self.hold_space(None);
        }
    }

    fn space_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        if key != gdk::Key::space || !self.space_navigates() {
            return glib::Propagation::Proceed;
        }
        self.hold_space(Some(modifiers));
        glib::Propagation::Stop
    }

    fn space_modifiers(&self, modifiers: gdk::ModifierType) -> glib::Propagation {
        if self
            .imp()
            .canvas
            .get()
            .is_some_and(|canvas| canvas.space_mode().is_some())
        {
            self.hold_space(Some(modifiers));
        }
        glib::Propagation::Proceed
    }

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
