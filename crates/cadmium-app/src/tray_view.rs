use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::{Surface, TRAY_HEIGHT, TRAY_WIDTH};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::canvas_widget::{
    CursorOutline, SurfaceScaleWatch, append_ring, device_scale, viewport_px,
};
use crate::input::PaintSurfaceWidget;
use crate::presenter::{FramePresenter, PresentSource, tray_view};
use crate::state::{AppState, LoadRing, SharedState};
use crate::view;

pub const RING_START_GAP_PX: f32 = 30.0;
pub const RING_MIN_WIDTH_PX: f32 = 2.0;
pub const RING_MAX_WIDTH_PX: f32 = 8.0;
pub const RING_MIN_ALPHA: f32 = 0.35;
pub const RING_ALPHA_RANGE: f32 = 0.6;
pub const RING_FADE_SECONDS: f32 = 0.4;
const TRAY_CURSOR: &str = "crosshair";
const TRAY_MIN_HEIGHT: i32 = 120;
const CHANNEL_MAX: f32 = 255.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RingShape {
    pub radius: f32,
    pub width: f32,
    pub alpha: f32,
}

pub fn ring_shape(outline_radius: f32, ring: &LoadRing) -> RingShape {
    let level = ring.level.clamp(0.0, 1.0);
    let pressure = ring.pressure.clamp(0.0, 1.0);
    RingShape {
        radius: outline_radius + RING_START_GAP_PX * (1.0 - level),
        width: RING_MIN_WIDTH_PX + (RING_MAX_WIDTH_PX - RING_MIN_WIDTH_PX) * pressure,
        alpha: RING_MIN_ALPHA + RING_ALPHA_RANGE * level,
    }
}

pub fn fading_ring(shape: RingShape, seconds_since_release: f32) -> Option<RingShape> {
    let remaining = 1.0 - seconds_since_release.max(0.0) / RING_FADE_SECONDS;
    (remaining > 0.0).then_some(RingShape {
        alpha: shape.alpha * remaining,
        ..shape
    })
}

pub fn tray_point(
    viewport_px: (u32, u32),
    device_scale: f64,
    widget_x: f64,
    widget_y: f64,
) -> (f64, f64) {
    view::viewport_to_canvas(
        tray_view(viewport_px),
        widget_x * device_scale,
        widget_y * device_scale,
    )
}

pub fn is_on_tray((x, y): (f64, f64)) -> bool {
    (0.0..f64::from(TRAY_WIDTH)).contains(&x) && (0.0..f64::from(TRAY_HEIGHT)).contains(&y)
}

pub fn tray_screen_zoom(viewport_px: (u32, u32), device_scale: f64) -> f32 {
    tray_view(viewport_px).zoom / device_scale as f32
}

#[derive(Clone, Copy, Debug)]
pub struct ShownRing {
    ring: LoadRing,
    centre: (f64, f64),
    released_at: Option<Instant>,
}

type TrayHandler = Box<dyn Fn(&TrayView)>;
type PickHandler = Box<dyn Fn(&TrayView, bool)>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct TrayView {
        pub state: OnceCell<SharedState>,
        pub presenter: RefCell<FramePresenter>,
        pub texture: RefCell<Option<gdk::Texture>>,
        pub needs_present: Cell<bool>,
        pub tick: RefCell<Option<gtk::TickCallbackId>>,
        pub scale_watch: SurfaceScaleWatch,
        pub hover: Cell<Option<(f64, f64)>>,
        pub shown_ring: Cell<Option<ShownRing>>,
        pub released_handlers: RefCell<Vec<TrayHandler>>,
        pub colour_picked_handlers: RefCell<Vec<PickHandler>>,
        pub pen_observed_handlers: RefCell<Vec<TrayHandler>>,
        pub fallback_handlers: RefCell<Vec<TrayHandler>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TrayView {
        const NAME: &'static str = "CadmiumTrayView";
        type Type = super::TrayView;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for TrayView {
        fn constructed(&self) {
            self.parent_constructed();
            self.presenter
                .replace(FramePresenter::showing(PresentSource::Tray));
            let tray = self.obj();
            tray.set_hexpand(true);
            tray.set_vexpand(true);
            tray.set_overflow(gtk::Overflow::Hidden);
            tray.set_cursor_from_name(Some(TRAY_CURSOR));
            tray.set_size_request(-1, TRAY_MIN_HEIGHT);
            tray.add_css_class("tray");
            tray.connect_scale_factor_notify(super::TrayView::follow_scale_change);
        }
    }

    impl WidgetImpl for TrayView {
        fn realize(&self) {
            self.parent_realize();
            let tray = self.obj();
            self.scale_watch
                .start(&*tray, super::TrayView::follow_scale_change);
            tray.queue_present();
        }

        fn unrealize(&self) {
            self.scale_watch.stop();
            self.texture.replace(None);
            self.presenter.borrow_mut().release();
            self.parent_unrealize();
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            self.obj().follow_scale_change();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let tray = self.obj();
            let scale = tray.device_scale();
            if let Some(texture) = self.texture.borrow().as_ref() {
                let size = (texture.width() as u32, texture.height() as u32);
                let to_widget = tray_screen_zoom(size, scale);
                let bounds = graphene::Rect::new(
                    0.0,
                    0.0,
                    size.0 as f32 / scale as f32,
                    size.1 as f32 / scale as f32,
                );
                let image = graphene::Rect::new(
                    0.0,
                    0.0,
                    TRAY_WIDTH as f32 * to_widget,
                    TRAY_HEIGHT as f32 * to_widget,
                );
                snapshot.push_clip(&image);
                snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Linear, &bounds);
                snapshot.pop();
            }
            let outline = tray.cursor_outline();
            if let Some((x, y)) = self.hover.get()
                && let Some(outline) = outline.as_ref()
                && outline.is_visible()
            {
                outline.append_to(snapshot, (x as f32, y as f32));
            }
            if let Some(shown) = self.shown_ring.get()
                && let Some(outline) = outline.as_ref()
            {
                let shape = ring_shape(outline.radius(), &shown.ring);
                let shape = match shown.released_at {
                    None => Some(shape),
                    Some(released) => fading_ring(shape, released.elapsed().as_secs_f32()),
                };
                if let Some(shape) = shape {
                    let (x, y) = shown.centre;
                    append_ring(
                        snapshot,
                        (x as f32, y as f32),
                        [shape.radius; 2],
                        0.0,
                        paint_colour(shown.ring.colour, shape.alpha),
                        shape.width,
                    );
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct TrayView(ObjectSubclass<imp::TrayView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

fn paint_colour([red, green, blue]: [u8; 3], alpha: f32) -> gdk::RGBA {
    gdk::RGBA::new(
        f32::from(red) / CHANNEL_MAX,
        f32::from(green) / CHANNEL_MAX,
        f32::from(blue) / CHANNEL_MAX,
        alpha,
    )
}

impl TrayView {
    pub fn new(state: SharedState) -> Self {
        let tray: TrayView = glib::Object::new();
        let weak = tray.downgrade();
        state.borrow_mut().add_waker(Rc::new(move || {
            if let Some(tray) = weak.upgrade() {
                tray.queue_present();
            }
        }));
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

    pub fn connect_presentation_fallback(&self, handler: impl Fn(&TrayView) + 'static) {
        self.imp()
            .fallback_handlers
            .borrow_mut()
            .push(Box::new(handler));
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
        let tick = self.add_tick_callback(|tray, _| {
            tray.present_frame();
            let ring_moving = tray.follow_load_ring();
            if ring_moving || tray.wants_frame() {
                return glib::ControlFlow::Continue;
            }
            tray.imp().tick.take();
            glib::ControlFlow::Break
        });
        imp.tick.replace(Some(tick));
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

    fn follow_load_ring(&self) -> bool {
        let imp = self.imp();
        let ring = imp
            .state
            .get()
            .and_then(|state| state.try_borrow().ok().map(|state| state.load_ring()));
        let Some(ring) = ring else {
            return imp.shown_ring.get().is_some();
        };
        let shown = imp.shown_ring.get();
        let next = match (ring, shown) {
            (Some(ring), _) => imp
                .hover
                .get()
                .or(shown.map(|shown| shown.centre))
                .map(|centre| ShownRing {
                    ring,
                    centre,
                    released_at: None,
                }),
            (None, Some(shown)) => match shown.released_at {
                None => Some(ShownRing {
                    released_at: Some(Instant::now()),
                    ..shown
                }),
                Some(released) if released.elapsed().as_secs_f32() < RING_FADE_SECONDS => {
                    Some(shown)
                }
                Some(_) => None,
            },
            (None, None) => None,
        };
        imp.shown_ring.set(next);
        if shown.is_some() || next.is_some() {
            self.queue_draw();
        }
        next.is_some()
    }

    fn frame_needed(&self, state: &AppState, presenter: &FramePresenter) -> bool {
        let (width_px, height_px) = viewport_px(self);
        if width_px == 0 || height_px == 0 || state.gpu_is_lost() {
            return false;
        }
        self.imp().needs_present.get()
            || state.session.tray_has_pending_work()
            || state.holds_on(Surface::Tray)
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

    fn present_frame(&self) {
        let imp = self.imp();
        let fell_back = self.present_texture() && imp.presenter.borrow_mut().take_fallback_notice();
        if fell_back {
            for handler in imp.fallback_handlers.borrow().iter() {
                handler(self);
            }
        }
    }

    fn present_texture(&self) -> bool {
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
        let (width_px, height_px) = viewport_px(self);
        state.set_tray_zoom(tray_screen_zoom((width_px, height_px), self.device_scale()));
        let Some(texture) = presenter.present(&mut state, &native, width_px, height_px) else {
            return false;
        };
        imp.needs_present.set(false);
        imp.texture.replace(Some(texture));
        self.queue_draw();
        true
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
        for handler in self.imp().pen_observed_handlers.borrow().iter() {
            handler(self);
        }
    }

    fn contact_released(&self) {
        for handler in self.imp().released_handlers.borrow().iter() {
            handler(self);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTLINE: f32 = 12.0;
    const TOLERANCE: f32 = 1e-5;

    fn ring(level: f32, pressure: f32) -> LoadRing {
        LoadRing {
            level,
            pressure,
            colour: [180, 70, 40],
        }
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < TOLERANCE,
            "{actual} is not {expected}"
        );
    }

    #[test]
    fn an_empty_brush_rings_wide_and_faint() {
        let shape = ring_shape(OUTLINE, &ring(0.0, 0.5));
        assert_close(shape.radius, OUTLINE + 30.0);
        assert_close(shape.alpha, 0.35);
    }

    #[test]
    fn a_full_brush_rings_on_its_outline() {
        let shape = ring_shape(OUTLINE, &ring(1.0, 0.5));
        assert_close(shape.radius, OUTLINE);
        assert_close(shape.alpha, 0.95);
    }

    #[test]
    fn pressure_thickens_the_ring() {
        assert_close(ring_shape(OUTLINE, &ring(0.5, 0.0)).width, 2.0);
        assert_close(ring_shape(OUTLINE, &ring(0.5, 1.0)).width, 8.0);
    }

    #[test]
    fn levels_outside_zero_to_one_are_clamped() {
        assert_eq!(
            ring_shape(OUTLINE, &ring(-0.5, -1.0)),
            ring_shape(OUTLINE, &ring(0.0, 0.0))
        );
        assert_eq!(
            ring_shape(OUTLINE, &ring(1.5, 2.0)),
            ring_shape(OUTLINE, &ring(1.0, 1.0))
        );
    }

    #[test]
    fn a_released_ring_fades_where_it_was() {
        let shape = ring_shape(OUTLINE, &ring(0.5, 0.5));
        let half_way = fading_ring(shape, RING_FADE_SECONDS / 2.0).expect("still fading");
        assert_close(half_way.alpha, shape.alpha / 2.0);
        assert_eq!(half_way.radius, shape.radius);
        assert_eq!(half_way.width, shape.width);
    }

    #[test]
    fn a_ring_is_gone_once_its_fade_ends() {
        let shape = ring_shape(OUTLINE, &ring(0.5, 0.5));
        assert_eq!(fading_ring(shape, RING_FADE_SECONDS), None);
    }

    #[test]
    fn only_points_on_the_tray_image_are_on_the_tray() {
        let (width, height) = (f64::from(TRAY_WIDTH), f64::from(TRAY_HEIGHT));
        for inside in [(0.0, 0.0), (width - 0.5, height - 0.5), (700.0, 300.0)] {
            assert!(is_on_tray(inside), "{inside:?}");
        }
        for outside in [
            (-0.5, 10.0),
            (10.0, -0.5),
            (width, 10.0),
            (10.0, height),
            (f64::NAN, 10.0),
        ] {
            assert!(!is_on_tray(outside), "{outside:?}");
        }
    }

    #[test]
    fn a_tall_widget_leaves_room_below_the_tray() {
        let viewport = (384, 400);
        assert!(is_on_tray(tray_point(viewport, 1.0, 383.0, 255.0)));
        assert!(!is_on_tray(tray_point(viewport, 1.0, 100.0, 300.0)));
    }

    #[test]
    fn the_widget_corners_are_the_tray_corners() {
        let viewport = (768, 512);
        assert_eq!(tray_point(viewport, 2.0, 0.0, 0.0), (0.0, 0.0));
        assert_eq!(tray_point(viewport, 2.0, 384.0, 256.0), (1536.0, 1024.0));
    }

    #[test]
    fn a_widget_pixel_covers_one_over_the_screen_zoom_tray_pixels() {
        for (viewport, scale) in [((384, 400), 1.0), ((600, 300), 1.5), ((768, 900), 2.0)] {
            let zoom = f64::from(tray_screen_zoom(viewport, scale));
            let (x0, y0) = tray_point(viewport, scale, 10.0, 20.0);
            let (x1, y1) = tray_point(viewport, scale, 11.0, 23.0);
            assert!(((x1 - x0) - 1.0 / zoom).abs() < 1e-3);
            assert!(((y1 - y0) - 3.0 / zoom).abs() < 1e-3);
        }
    }
}
