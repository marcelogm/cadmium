use std::cell::{Cell, OnceCell, RefCell};

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};
use oleo_engine::present::ViewTransform;

use crate::LOG_DOMAIN;
use crate::presenter::FramePresenter;
use crate::state::SharedState;
use crate::view;

const FIT_MARGIN: f64 = 24.0;
const ZOOM_STEP: f32 = 1.25;
const OUTLINE_WIDTH: f32 = 1.0;
const OUTLINE_MIN_RADIUS: f32 = 2.0;
const OUTLINE_LIGHT: gdk::RGBA = gdk::RGBA::new(1.0, 1.0, 1.0, 0.9);
const OUTLINE_DARK: gdk::RGBA = gdk::RGBA::new(0.0, 0.0, 0.0, 0.7);
const CANVAS_CURSOR: &str = "crosshair";

type ViewChangedHandler = Box<dyn Fn(&OleoCanvas)>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct OleoCanvas {
        pub state: OnceCell<SharedState>,
        pub presenter: RefCell<FramePresenter>,
        pub texture: RefCell<Option<gdk::Texture>>,
        pub needs_present: Cell<bool>,
        pub hover: Cell<Option<(f64, f64)>>,
        pub fitted: Cell<bool>,
        pub presented_frames: Cell<u64>,
        pub view_changed_handlers: RefCell<Vec<ViewChangedHandler>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for OleoCanvas {
        const NAME: &'static str = "OleoCanvas";
        type Type = super::OleoCanvas;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for OleoCanvas {
        fn constructed(&self) {
            self.parent_constructed();
            let canvas = self.obj();
            canvas.set_hexpand(true);
            canvas.set_vexpand(true);
            canvas.set_focusable(true);
            canvas.set_focus_on_click(true);
            canvas.set_overflow(gtk::Overflow::Hidden);
            canvas.set_cursor_from_name(Some(CANVAS_CURSOR));
            canvas.add_tick_callback(|canvas, _| {
                canvas.present_if_needed();
                glib::ControlFlow::Continue
            });
            canvas.connect_scale_factor_notify(|canvas| canvas.queue_present());
        }
    }

    impl WidgetImpl for OleoCanvas {
        fn map(&self) {
            self.parent_map();
            self.obj().grab_focus();
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            let canvas = self.obj();
            if !self.fitted.get() && width > 0 && height > 0 {
                self.fitted.set(true);
                canvas.zoom_fit();
            }
            canvas.queue_present();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let canvas = self.obj();
            let bounds =
                graphene::Rect::new(0.0, 0.0, canvas.width() as f32, canvas.height() as f32);
            if let Some(texture) = self.texture.borrow().as_ref() {
                snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Nearest, &bounds);
            }
            if let Some((x, y)) = self.hover.get()
                && let Some(radius) = canvas.outline_radius()
            {
                append_ring(
                    snapshot,
                    x as f32,
                    y as f32,
                    radius + OUTLINE_WIDTH,
                    OUTLINE_DARK,
                );
                append_ring(snapshot, x as f32, y as f32, radius, OUTLINE_LIGHT);
            }
        }
    }
}

glib::wrapper! {
    pub struct OleoCanvas(ObjectSubclass<imp::OleoCanvas>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

fn append_ring(snapshot: &gtk::Snapshot, x: f32, y: f32, radius: f32, colour: gdk::RGBA) {
    let bounds = graphene::Rect::new(x - radius, y - radius, 2.0 * radius, 2.0 * radius);
    let outline = gsk::RoundedRect::from_rect(bounds, radius);
    snapshot.append_border(&outline, &[OUTLINE_WIDTH; 4], &[colour; 4]);
}

impl OleoCanvas {
    pub fn new(state: SharedState) -> Self {
        let canvas: OleoCanvas = glib::Object::new();
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
        self.set_view(view::pan(current, dx * scale, dy * scale));
    }

    pub fn hover(&self) -> Option<(f64, f64)> {
        self.imp().hover.get()
    }

    pub fn set_hover(&self, position: Option<(f64, f64)>) {
        if self.imp().hover.replace(position) != position {
            self.queue_draw();
        }
    }

    pub fn connect_view_changed(&self, handler: impl Fn(&OleoCanvas) + 'static) {
        self.imp()
            .view_changed_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn pick_colour_at(&self, widget_x: f64, widget_y: f64) {
        let (x, y) = self.canvas_point(widget_x, widget_y);
        glib::g_message!(
            LOG_DOMAIN,
            "conta-gotas em ({x:.0}, {y:.0}): não implementado ainda"
        );
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

    fn outline_radius(&self) -> Option<f32> {
        let state = self.imp().state.get()?.try_borrow().ok()?;
        let canvas_radius = state.session.brush.geometry.max_diameter_px / 2.0;
        let radius = canvas_radius * state.view.zoom / self.device_scale() as f32;
        (radius >= OUTLINE_MIN_RADIUS).then_some(radius)
    }

    fn viewport_px(&self) -> (u32, u32) {
        let scale = self.device_scale();
        (
            (f64::from(self.width()) * scale).ceil() as u32,
            (f64::from(self.height()) * scale).ceil() as u32,
        )
    }

    fn present_if_needed(&self) {
        let imp = self.imp();
        let Some(state) = imp.state.get() else {
            return;
        };
        let (width_px, height_px) = self.viewport_px();
        if width_px == 0 || height_px == 0 {
            return;
        }
        let Ok(mut state) = state.try_borrow_mut() else {
            return;
        };
        let mut presenter = imp.presenter.borrow_mut();
        let needed = imp.needs_present.get()
            || state.session.has_pending_work()
            || !presenter.matches_size(width_px, height_px);
        if !needed {
            return;
        }
        imp.needs_present.set(false);
        let texture = presenter.present(&mut state, width_px, height_px);
        imp.texture.replace(Some(texture));
        imp.presented_frames.set(imp.presented_frames.get() + 1);
        self.queue_draw();
    }
}
