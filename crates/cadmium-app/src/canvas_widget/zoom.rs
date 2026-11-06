use cadmium_engine::present::ViewTransform;
use gtk::gdk;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::CadmiumCanvas;
use super::auto_fit::AutoFit;
use crate::cursor_outline::CursorOutline;
use crate::handlers::run_handlers;
use crate::input::nav_drag::{NavMode, held_mode};
use crate::surface_scale::device_scale;
use crate::view::{self, ViewStep};

const FIT_MARGIN: f64 = 24.0;
const ZOOM_STEP: f32 = 1.25;
const ROTATE_STEP: f32 = std::f32::consts::PI / 12.0;

impl CadmiumCanvas {
    pub fn device_scale(&self) -> f64 {
        device_scale(self)
    }

    pub fn zoom(&self) -> f32 {
        self.state().borrow().view.zoom
    }

    pub fn canvas_point(&self, widget_x: f64, widget_y: f64) -> (f64, f64) {
        let scale = self.device_scale();
        view::viewport_to_canvas(
            self.state().borrow().view,
            widget_x * scale,
            widget_y * scale,
        )
    }

    pub fn zoom_fit(&self) {
        let scale = self.device_scale();
        let spec_size = {
            let state = self.state();
            let state = state.borrow();
            (
                state.session.canvas.spec.width,
                state.session.canvas.spec.height,
            )
        };
        let viewport = (
            f64::from(self.width()) * scale,
            f64::from(self.height()) * scale,
        );
        self.set_view(view::fit_view(spec_size, viewport, FIT_MARGIN * scale));
        self.with_auto_fit(AutoFit::fitted);
    }

    pub fn zoom_in(&self) {
        self.apply_view_step(ViewStep::Zoom {
            anchor: self.widget_centre(),
            factor: ZOOM_STEP,
        });
    }

    pub fn zoom_out(&self) {
        // Kept off `ViewStep::Zoom` on purpose: `zoom * ZOOM_STEP.recip()` can differ from
        // `zoom / ZOOM_STEP` in the last bit.
        let (x, y) = self.widget_centre();
        let scale = self.device_scale();
        let current = self.state().borrow().view;
        self.with_auto_fit(AutoFit::moved_by_user);
        self.set_view(view::zoom_around(
            current,
            x * scale,
            y * scale,
            current.zoom / ZOOM_STEP,
        ));
    }

    pub fn pan_by(&self, dx: f64, dy: f64) {
        self.apply_view_step(ViewStep::Pan { dx, dy });
    }

    /// Turns the picture clockwise by `delta` radians about widget point (`widget_x`, `widget_y`),
    /// which stays put on screen.
    pub fn rotate_around(&self, widget_x: f64, widget_y: f64, delta: f32) {
        self.apply_view_step(ViewStep::Rotate {
            centre: (widget_x, widget_y),
            delta,
        });
    }

    /// Turns the picture one 15° step about the viewport centre, clockwise when `clockwise`,
    /// otherwise anticlockwise.
    pub fn rotate_step(&self, clockwise: bool) {
        let delta = if clockwise { ROTATE_STEP } else { -ROTATE_STEP };
        let (x, y) = self.widget_centre();
        self.rotate_around(x, y, delta);
    }

    /// The widget's centre, in widget px.
    pub fn widget_centre(&self) -> (f64, f64) {
        (
            f64::from(self.width()) / 2.0,
            f64::from(self.height()) / 2.0,
        )
    }

    /// Whether the window holds Space down for the canvas.
    pub(crate) fn space_held(&self) -> bool {
        self.imp().space_held.get()
    }

    /// Records which navigation keys are held (Space, and Ctrl or Shift among `modifiers`);
    /// the pointer shows the mode they have ready.
    pub(crate) fn set_held_keys(&self, space_held: bool, modifiers: gdk::ModifierType) {
        self.imp().space_held.set(space_held);
        let mode = held_mode(space_held, modifiers);
        if self.imp().held_mode.replace(mode) != mode {
            self.set_cursor_from_name(Some(
                mode.map_or(super::CANVAS_CURSOR, NavMode::cursor_name),
            ));
        }
    }

    /// Applies one step of a navigation gesture or key to the view.
    pub(crate) fn apply_view_step(&self, step: ViewStep) {
        let current = self.state().borrow().view;
        self.with_auto_fit(AutoFit::moved_by_user);
        self.show_view(view::step(current, step, self.device_scale()));
    }

    /// Settles `new_view`, then shows it.
    fn set_view(&self, new_view: ViewTransform) {
        self.show_view(view::settle(new_view));
    }

    /// Shows `new_view`, which is already settled. `view::step` settles already, and settling
    /// again would move the rotation (`normalise_rotation` isn't idempotent in f32), so user
    /// steps come here, not through `set_view`.
    fn show_view(&self, new_view: ViewTransform) {
        self.state().borrow_mut().view = new_view;
        self.queue_present();
        self.queue_draw();
        run_handlers(&self.imp().view_changed_handlers, self);
    }

    pub(super) fn cursor_outline(&self) -> Option<CursorOutline> {
        let state = self.imp().state.get()?.try_borrow().ok()?;
        let to_widget = state.view.zoom / self.device_scale() as f32;
        Some(CursorOutline::of_brush(&state, to_widget).turned(state.view.rotation))
    }

    pub(super) fn note_device_scale(&self) {
        if let Some(state) = self.imp().state.get()
            && let Ok(mut state) = state.try_borrow_mut()
        {
            state.set_painting_device_scale(self.device_scale() as f32);
        }
    }

    pub(super) fn follow_scale_change(&self) {
        self.note_device_scale();
        self.fit_if_due(self.width() > 0 && self.height() > 0);
        self.queue_present();
    }

    /// The widget's size or scale changed; `sized` when it has a size. Fits the view if the
    /// auto-fit rule says so.
    pub(super) fn fit_if_due(&self, sized: bool) {
        if self.with_auto_fit(|fit| fit.on_size_change(sized, || self.stroking())) {
            self.zoom_fit();
        }
    }

    /// Runs `f` on a copy of the auto-fit rule and stores the result back. `f` must not call
    /// `with_auto_fit`.
    pub(super) fn with_auto_fit<R>(&self, f: impl FnOnce(&mut AutoFit) -> R) -> R {
        let cell = &self.imp().auto_fit;
        let mut fit = cell.get();
        let result = f(&mut fit);
        cell.set(fit);
        result
    }
}
