use super::auto_fit::AutoFit;
use super::*;
use crate::input::nav_drag::NavMode;
use crate::provisional_overlay::append_provisional;

#[derive(Default)]
pub struct CadmiumCanvas {
    pub state: OnceCell<SharedState>,
    pub presenter: RefCell<FramePresenter>,
    pub texture: RefCell<Option<gdk::Texture>>,
    pub needs_present: Cell<bool>,
    pub tick: RefCell<Option<gtk::TickCallbackId>>,
    pub scale_watch: SurfaceScaleWatch,
    pub hover: Cell<Option<(f64, f64)>>,
    pub auto_fit: Cell<AutoFit>,
    pub allocated: Cell<(i32, i32)>,
    pub presented_frames: Cell<u64>,
    pub view_changed_handlers: RefCell<Vec<CanvasHandler>>,
    pub colour_picked_handlers: RefCell<Vec<PickHandler>>,
    pub pen_observed_handlers: RefCell<Vec<CanvasHandler>>,
    pub presented_handlers: RefCell<Vec<CanvasHandler>>,
    pub fallback_handlers: RefCell<Vec<CanvasHandler>>,
    pub gpu_lost_handlers: RefCell<Vec<CanvasHandler>>,
    pub released_handlers: RefCell<Vec<CanvasHandler>>,
    pub gpu_lost_announced: Cell<bool>,
    pub(crate) space_held: Cell<bool>,
    pub(crate) held_mode: Cell<Option<NavMode>>,
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
        self.scale_watch
            .start(&*canvas, super::CadmiumCanvas::follow_scale_change);
        canvas.queue_present();
    }

    fn unrealize(&self) {
        self.scale_watch.stop();
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
        if resized {
            canvas.fit_if_due(width > 0 && height > 0);
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
        if let Some((colour, marks)) = canvas.provisional_marks() {
            append_provisional(snapshot, colour, &marks);
        }
        if let Some((x, y)) = self.hover.get()
            && let Some(outline) = canvas.cursor_outline()
            && outline.is_visible()
        {
            outline.append_to(snapshot, (x as f32, y as f32));
        }
    }
}
