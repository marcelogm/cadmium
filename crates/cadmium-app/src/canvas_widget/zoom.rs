use cadmium_engine::present::ViewTransform;
use cadmium_engine::surface::Surface;
use gtk::gdk;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::CadmiumCanvas;
use crate::cursor_outline::CursorOutline;
use crate::handlers::run_handlers;
use crate::state::Mark;
use crate::state::stroking_or_busy;
use crate::surface_scale::{device_scale, widget_centre};
use crate::view::auto_fit::{AutoFit, update_auto_fit};
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
        let viewport = view::above_inset(
            (
                f64::from(self.width()) * scale,
                f64::from(self.height()) * scale,
            ),
            f64::from(self.imp().fit_inset.get()) * scale,
        );
        self.set_view(view::fit_view(spec_size, viewport, FIT_MARGIN * scale));
        update_auto_fit(&self.imp().auto_fit, AutoFit::fitted);
    }

    pub fn zoom_in(&self) {
        self.apply_view_step(ViewStep::Zoom {
            anchor: widget_centre(self),
            factor: ZOOM_STEP,
        });
    }

    pub fn zoom_out(&self) {
        // Kept off `ViewStep::Zoom` on purpose: `zoom * ZOOM_STEP.recip()` can differ from
        // `zoom / ZOOM_STEP` in the last bit.
        let (x, y) = widget_centre(self);
        let scale = self.device_scale();
        let current = self.state().borrow().view;
        update_auto_fit(&self.imp().auto_fit, AutoFit::moved_by_user);
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
        let (x, y) = widget_centre(self);
        self.rotate_around(x, y, delta);
    }

    /// Keeps fits clear of a bar `inset_px` tall over the canvas's foot (the tray's), and
    /// refits if the view is still the fitted one.
    pub fn set_fit_inset(&self, inset_px: i32) {
        if self.imp().fit_inset.replace(inset_px) != inset_px {
            self.fit_if_due(self.width() > 0 && self.height() > 0);
        }
    }

    /// Whether the window holds Space down for the canvas.
    pub(crate) fn space_held(&self) -> bool {
        self.imp().held_keys.space_held()
    }

    /// Records which navigation keys are held (Space, and Ctrl or Shift among `modifiers`);
    /// the pointer shows the mode they have ready.
    pub(crate) fn set_held_keys(&self, space_held: bool, modifiers: gdk::ModifierType) {
        let held = &self.imp().held_keys;
        if let Some(cursor) = held.set(space_held, modifiers, super::CANVAS_CURSOR) {
            self.set_cursor_from_name(Some(cursor));
        }
    }

    /// Applies one step of a navigation gesture or key to the view.
    pub(crate) fn apply_view_step(&self, step: ViewStep) {
        let current = self.state().borrow().view;
        update_auto_fit(&self.imp().auto_fit, AutoFit::moved_by_user);
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
        Some(
            CursorOutline::of_brush(&state, Surface::Painting, to_widget)
                .turned(state.view.rotation),
        )
    }

    pub(super) fn provisional_marks(&self) -> Option<([u8; 3], f32, Vec<Mark>)> {
        let state = self.imp().state.get()?.try_borrow().ok()?;
        let trail = state.provisional_trail()?;
        Some((
            trail.colour,
            trail.opacity,
            trail.widget_marks(self.device_scale() as f32),
        ))
    }

    pub(super) fn follow_scale_change(&self) {
        self.fit_if_due(self.width() > 0 && self.height() > 0);
        self.queue_present();
    }

    /// The widget's size or scale changed; `sized` when it has a size. Fits the view if the
    /// auto-fit rule says so.
    pub(super) fn fit_if_due(&self, sized: bool) {
        let stroking = || stroking_or_busy(self.imp().state.get());
        if update_auto_fit(&self.imp().auto_fit, |fit| {
            fit.on_size_change(sized, stroking)
        }) {
            self.zoom_fit();
        }
    }
}
