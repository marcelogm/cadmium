use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::Surface;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::handlers::run_handlers;
use crate::input::PaintSurfaceWidget;
use crate::presenter::FramePresenter;
use crate::state::SharedState;
use crate::surface_scale::SurfaceScaleWatch;

const CANVAS_CURSOR: &str = "crosshair";

type CanvasHandler = Box<dyn Fn(&CadmiumCanvas)>;
type PickHandler = Box<dyn Fn(&CadmiumCanvas, bool)>;

mod frames;
mod imp;
mod zoom;

glib::wrapper! {
    pub struct CadmiumCanvas(ObjectSubclass<imp::CadmiumCanvas>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl CadmiumCanvas {
    pub fn new(state: SharedState) -> Self {
        let canvas: CadmiumCanvas = glib::Object::new();
        let weak = canvas.downgrade();
        state
            .borrow_mut()
            .add_waker(Rc::new(move || wake_canvas(&weak)));
        if canvas.imp().state.set(state).is_err() {
            unreachable!("a fresh canvas has no state yet");
        }
        crate::input::attach(&canvas);
        canvas
    }

    pub fn state(&self) -> SharedState {
        self.imp()
            .state
            .get()
            .expect("canvas is created with its state")
            .clone()
    }

    pub fn presented_frames(&self) -> u64 {
        self.imp().presented_frames.get()
    }

    pub fn hover(&self) -> Option<(f64, f64)> {
        self.imp().hover.get()
    }

    pub fn connect_view_changed(&self, handler: impl Fn(&CadmiumCanvas) + 'static) {
        self.imp()
            .view_changed_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_colour_picked(&self, handler: impl Fn(&CadmiumCanvas, bool) + 'static) {
        self.imp()
            .colour_picked_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_pen_observed(&self, handler: impl Fn(&CadmiumCanvas) + 'static) {
        self.imp()
            .pen_observed_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_presented(&self, handler: impl Fn(&CadmiumCanvas) + 'static) {
        self.imp()
            .presented_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_presentation_fallback(&self, handler: impl Fn(&CadmiumCanvas) + 'static) {
        self.imp()
            .fallback_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_gpu_lost(&self, handler: impl Fn(&CadmiumCanvas) + 'static) {
        self.imp()
            .gpu_lost_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_released(&self, handler: impl Fn(&CadmiumCanvas) + 'static) {
        self.imp()
            .released_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }
}

impl PaintSurfaceWidget for CadmiumCanvas {
    fn state(&self) -> SharedState {
        CadmiumCanvas::state(self)
    }

    fn surface(&self) -> Surface {
        Surface::Painting
    }

    fn surface_point(&self, widget_x: f64, widget_y: f64) -> (f64, f64) {
        self.canvas_point(widget_x, widget_y)
    }

    fn set_hover(&self, position: Option<(f64, f64)>) {
        if self.imp().hover.replace(position) != position {
            self.queue_draw();
        }
    }

    fn pick_colour_at(&self, widget_x: f64, widget_y: f64) {
        let (x, y) = self.canvas_point(widget_x, widget_y);
        let found_paint =
            self.state()
                .borrow_mut()
                .pick_colour_on(Surface::Painting, x as f32, y as f32);
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

/// Whether the canvas is still there to wake.
fn wake_canvas(canvas: &glib::WeakRef<CadmiumCanvas>) -> bool {
    let Some(canvas) = canvas.upgrade() else {
        return false;
    };
    canvas.request_frame();
    canvas.queue_draw();
    true
}
