use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::{Surface, TRAY_HEIGHT, TRAY_WIDTH};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use self::geometry::{is_on_tray, tray_height_for_width, tray_point, tray_screen_zoom};
use self::ring::{ShownRing, append_load_ring};
use crate::cursor_outline::CursorOutline;
use crate::handlers::run_handlers;
use crate::input::PaintSurfaceWidget;
use crate::presenter::{FramePresenter, PresentSource};
use crate::state::SharedState;
use crate::surface_scale::{SurfaceScaleWatch, device_scale, viewport_px};

const TRAY_CURSOR: &str = "crosshair";
const TRAY_FRAME_CLASS: &str = "card";
const TRAY_MIN_WIDTH: i32 = 180;

type TrayHandler = Box<dyn Fn(&TrayView)>;
type PickHandler = Box<dyn Fn(&TrayView, bool)>;

mod frames;
mod geometry;
mod imp;
mod ring;

glib::wrapper! {
    pub struct TrayView(ObjectSubclass<imp::TrayView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl TrayView {
    pub fn new(state: SharedState) -> Self {
        let tray: TrayView = glib::Object::new();
        let weak = tray.downgrade();
        state
            .borrow_mut()
            .add_waker(Rc::new(move || wake_tray(&weak)));
        if tray.imp().state.set(state).is_err() {
            unreachable!("a fresh tray has no state yet");
        }
        crate::input::attach_stylus(&tray);
        tray
    }

    pub fn state(&self) -> SharedState {
        self.imp()
            .state
            .get()
            .expect("tray is created with its state")
            .clone()
    }

    pub fn connect_released(&self, handler: impl Fn(&TrayView) + 'static) {
        self.imp()
            .released_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_colour_picked(&self, handler: impl Fn(&TrayView, bool) + 'static) {
        self.imp()
            .colour_picked_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_pen_observed(&self, handler: impl Fn(&TrayView) + 'static) {
        self.imp()
            .pen_observed_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_presented(&self, handler: impl Fn(&TrayView) + 'static) {
        self.imp()
            .presented_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_presentation_fallback(&self, handler: impl Fn(&TrayView) + 'static) {
        self.imp()
            .fallback_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    fn device_scale(&self) -> f64 {
        device_scale(self)
    }

    fn follow_scale_change(&self) {
        self.sync_tray_zoom();
        self.queue_present();
    }

    fn sync_tray_zoom(&self) {
        let Some(state) = self.imp().state.get() else {
            return;
        };
        if let Ok(mut state) = state.try_borrow_mut() {
            state.set_tray_zoom(tray_screen_zoom(viewport_px(self), self.device_scale()));
        }
    }

    fn cursor_outline(&self) -> Option<CursorOutline> {
        let state = self.imp().state.get()?.try_borrow().ok()?;
        let to_widget = tray_screen_zoom(viewport_px(self), self.device_scale());
        Some(CursorOutline::of_brush(&state, to_widget))
    }
}

impl PaintSurfaceWidget for TrayView {
    fn state(&self) -> SharedState {
        TrayView::state(self)
    }

    fn surface(&self) -> Surface {
        Surface::Tray
    }

    fn surface_point(&self, widget_x: f64, widget_y: f64) -> (f64, f64) {
        tray_point(viewport_px(self), self.device_scale(), widget_x, widget_y)
    }

    fn accepts_point(&self, widget_x: f64, widget_y: f64) -> bool {
        is_on_tray(self.surface_point(widget_x, widget_y))
    }

    fn claims_strokes(&self) -> bool {
        true
    }

    fn set_hover(&self, position: Option<(f64, f64)>) {
        let position = position.filter(|&(x, y)| self.accepts_point(x, y));
        if self.imp().hover.replace(position) != position {
            self.queue_draw();
        }
    }

    fn pick_colour_at(&self, widget_x: f64, widget_y: f64) {
        let (x, y) = self.surface_point(widget_x, widget_y);
        let found_paint =
            self.state()
                .borrow_mut()
                .pick_colour_on(Surface::Tray, x as f32, y as f32);
        for handler in self.imp().colour_picked_handlers.borrow().iter() {
            handler(self, found_paint);
        }
    }

    fn observe_hover_pen(&self, sample: &StrokeSample) {
        self.state().borrow_mut().observe_hover_pen(sample);
        run_handlers(&self.imp().pen_observed_handlers, self);
    }

    fn contact_released(&self) {
        run_handlers(&self.imp().released_handlers, self);
    }
}

/// Whether the tray is still there to wake.
fn wake_tray(tray: &glib::WeakRef<TrayView>) -> bool {
    let Some(tray) = tray.upgrade() else {
        return false;
    };
    tray.request_frame();
    tray.queue_draw();
    true
}
