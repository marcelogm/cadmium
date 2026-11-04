use cadmium_engine::present::ViewTransform;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::{CadmiumCanvas, run_handlers};
use crate::cursor_outline::CursorOutline;
use crate::surface_scale::device_scale;
use crate::view;

const FIT_MARGIN: f64 = 24.0;
const ZOOM_STEP: f32 = 1.25;

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
        self.imp().user_moved_view.set(false);
    }

    pub fn zoom_in(&self) {
        self.zoom_around_center(self.zoom() * ZOOM_STEP);
    }

    pub fn zoom_out(&self) {
        self.zoom_around_center(self.zoom() / ZOOM_STEP);
    }

    pub fn set_zoom_around(&self, widget_x: f64, widget_y: f64, zoom: f32) {
        let scale = self.device_scale();
        let current = self.state().borrow().view;
        self.imp().user_moved_view.set(true);
        self.set_view(view::zoom_around(
            current,
            widget_x * scale,
            widget_y * scale,
            zoom,
        ));
    }

    pub fn pan_by(&self, dx: f64, dy: f64) {
        let scale = self.device_scale();
        let current = self.state().borrow().view;
        self.imp().user_moved_view.set(true);
        self.set_view(view::pan(current, dx * scale, dy * scale));
    }

    fn zoom_around_center(&self, zoom: f32) {
        self.set_zoom_around(
            f64::from(self.width()) / 2.0,
            f64::from(self.height()) / 2.0,
            zoom,
        );
    }

    fn set_view(&self, new_view: ViewTransform) {
        let new_view = ViewTransform {
            zoom: view::clamp_zoom(new_view.zoom),
            ..new_view
        };
        self.state().borrow_mut().view = new_view;
        self.queue_present();
        self.queue_draw();
        run_handlers(&self.imp().view_changed_handlers, self);
    }

    pub(super) fn cursor_outline(&self) -> Option<CursorOutline> {
        let state = self.imp().state.get()?.try_borrow().ok()?;
        let to_widget = state.view.zoom / self.device_scale() as f32;
        Some(CursorOutline::of_brush(&state, to_widget))
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
        if !self.imp().user_moved_view.get() && self.width() > 0 && self.height() > 0 {
            self.fit_unless_stroking();
        }
        self.queue_present();
    }

    pub(super) fn fit_unless_stroking(&self) {
        let stroking = self
            .imp()
            .state
            .get()
            .and_then(|state| state.try_borrow().ok().map(|state| state.is_stroking()))
            .unwrap_or(true);
        if stroking {
            self.imp().fit_after_stroke.set(true);
        } else {
            self.imp().fit_after_stroke.set(false);
            self.zoom_fit();
        }
    }

    pub(super) fn fit_if_a_stroke_held_it_back(&self) {
        let imp = self.imp();
        if imp.fit_after_stroke.get() && !imp.user_moved_view.get() {
            self.fit_unless_stroking();
        }
    }
}
