//! The mixing sheet on screen: the pen paints on it as on the painting, but it never
//! navigates; the whole sheet is fitted in the widget.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use cadmium_engine::present::ViewTransform;
use cadmium_engine::sheet::{SHEET_HEIGHT_PX, SHEET_WIDTH_PX};
use cadmium_engine::stroke::StrokeSample;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::cursor_outline::CursorOutline;
use crate::handlers::{Handler, run_handlers};
use crate::input::{PaintSurfaceWidget, attach_stylus};
use crate::presenter::{FramePresenter, PresentSource};
use crate::state::{EaselId, SharedState};
use crate::surface_scale::{SurfaceScaleWatch, device_scale, viewport_px};
use crate::view::{fit_view, viewport_to_canvas};

const SHEET_CURSOR: &str = "crosshair";
const SHEET_FRAME_CLASS: &str = "card";
const SHEET_MIN_SIZE: (i32, i32) = (240, 160);

type PickHandler = Box<dyn Fn(&SheetView, bool)>;

mod frames;
mod imp;

glib::wrapper! {
    pub struct SheetView(ObjectSubclass<imp::SheetView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SheetView {
    pub fn new(state: SharedState) -> Self {
        let view: SheetView = glib::Object::new();
        let weak = view.downgrade();
        state
            .borrow_mut()
            .add_waker(Rc::new(move || wake_view(&weak)));
        if view.imp().state.set(state).is_err() {
            unreachable!("a fresh sheet view has no state yet");
        }
        attach_stylus(&view);
        view
    }

    pub fn state(&self) -> SharedState {
        self.imp()
            .state
            .get()
            .expect("the sheet view is created with its state")
            .clone()
    }

    pub fn connect_released(&self, handler: impl Fn(&SheetView) + 'static) {
        push(&self.imp().released_handlers, handler);
    }

    /// `handler` hears whether a pick on the sheet found paint.
    pub fn connect_colour_picked(&self, handler: impl Fn(&SheetView, bool) + 'static) {
        self.imp()
            .colour_picked_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_presented(&self, handler: impl Fn(&SheetView) + 'static) {
        push(&self.imp().presented_handlers, handler);
    }

    pub fn connect_gpu_lost(&self, handler: impl Fn(&SheetView) + 'static) {
        push(&self.imp().gpu_lost_handlers, handler);
    }

    pub fn connect_presentation_fallback(&self, handler: impl Fn(&SheetView) + 'static) {
        push(&self.imp().fallback_handlers, handler);
    }

    /// The view that fits the whole sheet in the widget, in device px.
    fn fitted_view(&self) -> ViewTransform {
        let (width_px, height_px) = viewport_px(self);
        let viewport = (f64::from(width_px), f64::from(height_px));
        fit_view((SHEET_WIDTH_PX, SHEET_HEIGHT_PX), viewport, 0.0)
    }

    fn cursor_outline(&self) -> Option<CursorOutline> {
        let state = self.imp().state.get()?.try_borrow().ok()?;
        let to_widget = self.fitted_view().zoom / device_scale(self) as f32;
        Some(CursorOutline::of_brush(&state, EaselId::Sheet, to_widget))
    }
}

impl PaintSurfaceWidget for SheetView {
    fn state(&self) -> SharedState {
        SheetView::state(self)
    }

    fn easel(&self) -> EaselId {
        EaselId::Sheet
    }

    fn surface_point(&self, widget_x: f64, widget_y: f64) -> (f64, f64) {
        let scale = device_scale(self);
        viewport_to_canvas(self.fitted_view(), widget_x * scale, widget_y * scale)
    }

    fn accepts_point(&self, widget_x: f64, widget_y: f64) -> bool {
        is_on_sheet(self.surface_point(widget_x, widget_y))
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
        let found_paint = self
            .state()
            .borrow_mut()
            .pick_colour(EaselId::Sheet, x as f32, y as f32);
        for handler in self.imp().colour_picked_handlers.borrow().iter() {
            handler(self, found_paint);
        }
    }

    fn observe_hover_pen(&self, sample: &StrokeSample) {
        self.state().borrow_mut().observe_hover_pen(sample);
        self.queue_draw();
    }

    fn contact_released(&self) {
        run_handlers(&self.imp().released_handlers, self);
    }
}

#[cfg(all(target_os = "linux", any(debug_assertions, feature = "smoke")))]
impl SheetView {
    /// Whether the sheet has shown a frame.
    pub(crate) fn has_frame(&self) -> bool {
        self.imp().texture.borrow().is_some()
    }

    /// Whether the sheet still presents frames: painting, settling, or catching up with its
    /// size.
    pub(crate) fn is_presenting(&self) -> bool {
        self.imp().tick.borrow().is_some()
    }
}

fn push(handlers: &RefCell<Vec<Handler<SheetView>>>, handler: impl Fn(&SheetView) + 'static) {
    handlers.borrow_mut().push(Box::new(handler));
}

fn is_on_sheet((x, y): (f64, f64)) -> bool {
    (0.0..f64::from(SHEET_WIDTH_PX)).contains(&x) && (0.0..f64::from(SHEET_HEIGHT_PX)).contains(&y)
}

/// Whether the view is still there to wake.
fn wake_view(view: &glib::WeakRef<SheetView>) -> bool {
    let Some(view) = view.upgrade() else {
        return false;
    };
    view.request_frame();
    view.queue_draw();
    true
}

fn append_sheet_image(snapshot: &gtk::Snapshot, texture: &gdk::Texture, scale: f64) {
    let bounds = graphene::Rect::new(
        0.0,
        0.0,
        texture.width() as f32 / scale as f32,
        texture.height() as f32 / scale as f32,
    );
    snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Linear, &bounds);
}
