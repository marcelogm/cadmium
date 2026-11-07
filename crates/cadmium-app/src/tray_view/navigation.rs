use cadmium_engine::present::ViewTransform;
use gtk::gdk;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::geometry::tray_cover;
use super::{TRAY_CURSOR, TrayView};
use crate::input::nav_drag::{NavMode, held_mode};
use crate::surface_scale::viewport_px;
use crate::view::auto_fit::AutoFit;
use crate::view::{self, ViewStep};

impl TrayView {
    /// Shows the whole pane filled with the tray, centred, and lets resizes refit it again.
    pub fn fit_view(&self) {
        let (width_px, height_px) = viewport_px(self);
        if width_px == 0 || height_px == 0 {
            return;
        }
        self.show_view(tray_cover((width_px, height_px)));
        self.with_auto_fit(AutoFit::fitted);
    }

    /// Applies one step of a navigation gesture to the tray's view.
    pub(crate) fn apply_view_step(&self, step: ViewStep) {
        let Some(current) = self.view() else {
            return;
        };
        self.with_auto_fit(AutoFit::moved_by_user);
        self.show_view(view::step(current, step, self.device_scale()));
    }

    /// The widget's centre, in widget px.
    pub(crate) fn widget_centre(&self) -> (f64, f64) {
        (
            f64::from(self.width()) / 2.0,
            f64::from(self.height()) / 2.0,
        )
    }

    /// Whether the window holds Space down for the tray.
    pub(crate) fn space_held(&self) -> bool {
        self.imp().space_held.get()
    }

    /// Records which navigation keys are held (Space, and Ctrl or Shift among `modifiers`);
    /// the pointer shows the mode they have ready.
    pub(crate) fn set_held_keys(&self, space_held: bool, modifiers: gdk::ModifierType) {
        self.imp().space_held.set(space_held);
        let mode = held_mode(space_held, modifiers);
        if self.imp().held_mode.replace(mode) != mode {
            self.set_cursor_from_name(Some(mode.map_or(TRAY_CURSOR, NavMode::cursor_name)));
        }
    }

    /// The widget's size or scale changed: refits while the painter hasn't moved the view,
    /// otherwise keeps the view on the new scale.
    pub(super) fn follow_scale_change(&self) {
        let sized = self.width() > 0 && self.height() > 0;
        if self.with_auto_fit(|fit| fit.on_size_change(sized, || self.stroking())) {
            self.fit_view();
        } else if let Some(current) = self.view() {
            self.show_view(current);
        }
        self.queue_present();
    }

    /// Makes the fit a stroke held back, once the stroke is over.
    pub(super) fn fit_if_held_back(&self) {
        if self.with_auto_fit(|fit| fit.on_frame(|| self.stroking())) {
            self.fit_view();
        }
    }

    fn view(&self) -> Option<ViewTransform> {
        let state = self.imp().state.get()?.try_borrow().ok()?;
        Some(state.tray_view)
    }

    fn show_view(&self, new_view: ViewTransform) {
        let Some(state) = self.imp().state.get() else {
            return;
        };
        let Ok(mut state) = state.try_borrow_mut() else {
            return;
        };
        state.set_tray_view(new_view, self.device_scale());
        drop(state);
        self.queue_present();
        self.queue_draw();
    }

    /// Whether a stroke is running; a borrowed or missing state counts as one.
    fn stroking(&self) -> bool {
        self.imp()
            .state
            .get()
            .and_then(|state| state.try_borrow().ok().map(|state| state.is_stroking()))
            .unwrap_or(true)
    }

    /// Runs `f` on a copy of the auto-fit rule and stores the result back. `f` must not call
    /// `with_auto_fit`.
    fn with_auto_fit<R>(&self, f: impl FnOnce(&mut AutoFit) -> R) -> R {
        let cell = &self.imp().auto_fit;
        let mut fit = cell.get();
        let result = f(&mut fit);
        cell.set(fit);
        result
    }
}
