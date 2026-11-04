use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use cadmium_engine::present::ViewTransform;
use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::Surface;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::input::PaintSurfaceWidget;
use crate::presenter::FramePresenter;
use crate::state::{AppState, SharedState};
use crate::tools::UiTool;
use crate::view;

const FIT_MARGIN: f64 = 24.0;
const ZOOM_STEP: f32 = 1.25;
const OUTLINE_WIDTH: f32 = 1.0;
const OUTLINE_MIN_RADIUS: f32 = 2.0;
const OUTLINE_LIGHT: gdk::RGBA = gdk::RGBA::new(1.0, 1.0, 1.0, 0.9);
const OUTLINE_DARK: gdk::RGBA = gdk::RGBA::new(0.0, 0.0, 0.0, 0.7);
const CANVAS_CURSOR: &str = "crosshair";

type CanvasHandler = Box<dyn Fn(&CadmiumCanvas)>;
type PickHandler = Box<dyn Fn(&CadmiumCanvas, bool)>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct CadmiumCanvas {
        pub state: OnceCell<SharedState>,
        pub presenter: RefCell<FramePresenter>,
        pub texture: RefCell<Option<gdk::Texture>>,
        pub needs_present: Cell<bool>,
        pub tick: RefCell<Option<gtk::TickCallbackId>>,
        pub scale_watch: RefCell<Option<(glib::WeakRef<gdk::Surface>, glib::SignalHandlerId)>>,
        pub hover: Cell<Option<(f64, f64)>>,
        pub user_moved_view: Cell<bool>,
        pub fit_after_stroke: Cell<bool>,
        pub allocated: Cell<(i32, i32)>,
        pub presented_frames: Cell<u64>,
        pub view_changed_handlers: RefCell<Vec<CanvasHandler>>,
        pub colour_picked_handlers: RefCell<Vec<PickHandler>>,
        pub pen_observed_handlers: RefCell<Vec<CanvasHandler>>,
        pub presented_handlers: RefCell<Vec<CanvasHandler>>,
        pub fallback_handlers: RefCell<Vec<CanvasHandler>>,
        pub gpu_lost_handlers: RefCell<Vec<CanvasHandler>>,
        pub gpu_lost_announced: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CadmiumCanvas {
        const NAME: &'static str = "CadmiumCanvas";
        type Type = super::CadmiumCanvas;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for CadmiumCanvas {
        fn constructed(&self) {
            self.parent_constructed();
            let canvas = self.obj();
            canvas.set_hexpand(true);
            canvas.set_vexpand(true);
            canvas.set_focusable(true);
            canvas.set_focus_on_click(true);
            canvas.set_overflow(gtk::Overflow::Hidden);
            canvas.set_cursor_from_name(Some(CANVAS_CURSOR));
            canvas.connect_scale_factor_notify(|canvas| canvas.follow_scale_change());
        }
    }

    impl WidgetImpl for CadmiumCanvas {
        fn realize(&self) {
            self.parent_realize();
            let canvas = self.obj();
            if let Some(surface) = canvas.native().and_then(|native| native.surface()) {
                let handler = surface.connect_scale_notify(glib::clone!(
                    #[weak]
                    canvas,
                    move |_| canvas.follow_scale_change()
                ));
                self.scale_watch
                    .replace(Some((surface.downgrade(), handler)));
            }
            canvas.queue_present();
        }

        fn unrealize(&self) {
            if let Some((surface, handler)) = self.scale_watch.take()
                && let Some(surface) = surface.upgrade()
            {
                surface.disconnect(handler);
            }
            self.texture.replace(None);
            self.presenter.borrow_mut().release();
            self.parent_unrealize();
        }

        fn map(&self) {
            self.parent_map();
            self.obj().grab_focus();
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            let canvas = self.obj();
            let resized = self.allocated.replace((width, height)) != (width, height);
            if resized && !self.user_moved_view.get() && width > 0 && height > 0 {
                canvas.fit_unless_stroking();
            }
            canvas.queue_present();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let canvas = self.obj();
            if let Some(texture) = self.texture.borrow().as_ref() {
                let scale = canvas.device_scale() as f32;
                let bounds = graphene::Rect::new(
                    0.0,
                    0.0,
                    texture.width() as f32 / scale,
                    texture.height() as f32 / scale,
                );
                snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Nearest, &bounds);
            }
            if let Some((x, y)) = self.hover.get()
                && let Some(outline) = canvas.cursor_outline()
            {
                let [along, across] = outline.semi_axes;
                append_ring(
                    snapshot,
                    (x as f32, y as f32),
                    [along + OUTLINE_WIDTH, across + OUTLINE_WIDTH],
                    outline.rotation_deg,
                    OUTLINE_DARK,
                );
                append_ring(
                    snapshot,
                    (x as f32, y as f32),
                    outline.semi_axes,
                    outline.rotation_deg,
                    OUTLINE_LIGHT,
                );
            }
        }
    }
}

glib::wrapper! {
    pub struct CadmiumCanvas(ObjectSubclass<imp::CadmiumCanvas>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

struct CursorOutline {
    semi_axes: [f32; 2],
    rotation_deg: f32,
}

fn append_ring(
    snapshot: &gtk::Snapshot,
    (x, y): (f32, f32),
    [along, across]: [f32; 2],
    rotation_deg: f32,
    colour: gdk::RGBA,
) {
    snapshot.save();
    snapshot.translate(&graphene::Point::new(x, y));
    snapshot.rotate(rotation_deg);
    let bounds = graphene::Rect::new(-along, -across, 2.0 * along, 2.0 * across);
    let corner = graphene::Size::new(along, across);
    let outline = gsk::RoundedRect::new(bounds, corner, corner, corner, corner);
    snapshot.append_border(&outline, &[OUTLINE_WIDTH; 4], &[colour; 4]);
    snapshot.restore();
}

impl CadmiumCanvas {
    pub fn new(state: SharedState) -> Self {
        let canvas: CadmiumCanvas = glib::Object::new();
        let weak = canvas.downgrade();
        state.borrow_mut().add_waker(Rc::new(move || {
            if let Some(canvas) = weak.upgrade() {
                canvas.queue_present();
            }
        }));
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

    pub fn queue_present(&self) {
        self.imp().needs_present.set(true);
        self.request_frame();
    }

    fn request_frame(&self) {
        let imp = self.imp();
        if imp.tick.borrow().is_some() {
            return;
        }
        let tick = self.add_tick_callback(|canvas, _| {
            canvas.present_if_needed();
            if canvas.wants_frame() {
                return glib::ControlFlow::Continue;
            }
            canvas.imp().tick.take();
            glib::ControlFlow::Break
        });
        imp.tick.replace(Some(tick));
    }

    pub fn device_scale(&self) -> f64 {
        self.native()
            .and_then(|native| native.surface())
            .map_or(f64::from(self.scale_factor()), |surface| surface.scale())
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
        for handler in self.imp().view_changed_handlers.borrow().iter() {
            handler(self);
        }
    }

    fn cursor_outline(&self) -> Option<CursorOutline> {
        let state = self.imp().state.get()?.try_borrow().ok()?;
        let mut brush = state.session.brush.clone();
        if let UiTool::Paint(tool) = state.ui_tool {
            brush.tool = tool;
        }
        let pen = StrokeSample {
            x: 0.0,
            y: 0.0,
            pressure: state.pen.pressure,
            tilt_x_deg: state.pen.tilt_x,
            tilt_y_deg: state.pen.tilt_y,
            time_ms: 0,
        };
        let to_widget = state.view.zoom / self.device_scale() as f32;
        let canvas_radius = brush.geometry.max_diameter_px / 2.0;
        let (semi_axes, rotation_deg) = match state.session.engine.brush_outline(&brush, &pen) {
            Some(outline) => (outline.semi_axes, outline.orientation.to_degrees()),
            None => ([canvas_radius; 2], 0.0),
        };
        let semi_axes = semi_axes.map(|semi| semi * to_widget);
        (semi_axes[0].max(semi_axes[1]) >= OUTLINE_MIN_RADIUS).then_some(CursorOutline {
            semi_axes,
            rotation_deg,
        })
    }

    fn viewport_px(&self) -> (u32, u32) {
        let scale = self.device_scale();
        (
            (f64::from(self.width()) * scale).ceil() as u32,
            (f64::from(self.height()) * scale).ceil() as u32,
        )
    }

    fn follow_scale_change(&self) {
        if !self.imp().user_moved_view.get() && self.width() > 0 && self.height() > 0 {
            self.fit_unless_stroking();
        }
        self.queue_present();
    }

    fn fit_unless_stroking(&self) {
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

    fn fit_if_a_stroke_held_it_back(&self) {
        let imp = self.imp();
        if imp.fit_after_stroke.get() && !imp.user_moved_view.get() {
            self.fit_unless_stroking();
        }
    }

    fn frame_needed(&self, state: &AppState, presenter: &FramePresenter) -> bool {
        let (width_px, height_px) = self.viewport_px();
        if width_px == 0 || height_px == 0 || state.gpu_is_lost() {
            return false;
        }
        self.imp().needs_present.get()
            || state.session.has_pending_work()
            || state.holds_on(Surface::Painting)
            || !presenter.matches_size(width_px, height_px)
    }

    fn wants_frame(&self) -> bool {
        let imp = self.imp();
        let Some(state) = imp.state.get() else {
            return false;
        };
        let (Ok(state), Ok(presenter)) = (state.try_borrow(), imp.presenter.try_borrow()) else {
            return true;
        };
        self.frame_needed(&state, &presenter)
    }

    fn announce_if_gpu_lost(&self) {
        let imp = self.imp();
        let lost = imp
            .state
            .get()
            .and_then(|state| state.try_borrow().ok().map(|state| state.gpu_is_lost()))
            .unwrap_or(false);
        if lost && !imp.gpu_lost_announced.replace(true) {
            for handler in imp.gpu_lost_handlers.borrow().iter() {
                handler(self);
            }
        }
    }

    fn present_if_needed(&self) {
        self.fit_if_a_stroke_held_it_back();
        let presented = self.present_frame();
        self.announce_if_gpu_lost();
        if !presented {
            return;
        }
        let imp = self.imp();
        let fell_back = imp.presenter.borrow_mut().take_fallback_notice();
        if fell_back {
            for handler in imp.fallback_handlers.borrow().iter() {
                handler(self);
            }
        }
        for handler in imp.presented_handlers.borrow().iter() {
            handler(self);
        }
    }

    fn present_frame(&self) -> bool {
        let imp = self.imp();
        let Some(state) = imp.state.get() else {
            return false;
        };
        let Some(native) = self.native() else {
            return false;
        };
        let Ok(mut state) = state.try_borrow_mut() else {
            return false;
        };
        let mut presenter = imp.presenter.borrow_mut();
        if !self.frame_needed(&state, &presenter) {
            return false;
        }
        let (width_px, height_px) = self.viewport_px();
        let Some(texture) = presenter.present(&mut state, &native, width_px, height_px) else {
            return false;
        };
        imp.needs_present.set(false);
        imp.texture.replace(Some(texture));
        imp.presented_frames.set(imp.presented_frames.get() + 1);
        self.queue_draw();
        true
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
        for handler in self.imp().pen_observed_handlers.borrow().iter() {
            handler(self);
        }
    }
}
