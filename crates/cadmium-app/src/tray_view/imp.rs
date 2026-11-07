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
    pub(super) shown_ring: Cell<Option<ShownRing>>,
    pub released_handlers: RefCell<Vec<TrayHandler>>,
    pub colour_picked_handlers: RefCell<Vec<PickHandler>>,
    pub pen_observed_handlers: RefCell<Vec<TrayHandler>>,
    pub fallback_handlers: RefCell<Vec<TrayHandler>>,
    pub presented_handlers: RefCell<Vec<TrayHandler>>,
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
        tray.add_css_class(TRAY_FRAME_CLASS);
        tray.set_overflow(gtk::Overflow::Hidden);
        tray.set_cursor_from_name(Some(TRAY_CURSOR));
        tray.connect_scale_factor_notify(super::TrayView::follow_scale_change);
    }
}

impl WidgetImpl for TrayView {
    fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
        match orientation {
            gtk::Orientation::Horizontal => (TRAY_MIN_WIDTH, TRAY_MIN_WIDTH, -1, -1),
            _ => (tray_min_height(), tray_min_height(), -1, -1),
        }
    }

    fn realize(&self) {
        self.parent_realize();
        let tray = self.obj();
        self.scale_watch
            .start(&*tray, super::TrayView::follow_scale_change);
        tray.queue_present();
    }

    fn map(&self) {
        self.parent_map();
        self.obj().queue_present();
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
            append_tray_image(snapshot, texture, scale);
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
            append_load_ring(snapshot, &shown, outline);
        }
    }
}

fn append_tray_image(snapshot: &gtk::Snapshot, texture: &gdk::Texture, scale: f64) {
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
