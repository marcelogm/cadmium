use super::*;

#[derive(Default)]
pub struct StudioView {
    pub state: OnceCell<SharedState>,
    pub presenter: RefCell<FramePresenter>,
    pub texture: RefCell<Option<gdk::Texture>>,
    pub needs_present: Cell<bool>,
    pub tick: RefCell<Option<gtk::TickCallbackId>>,
    pub scale_watch: SurfaceScaleWatch,
    /// The drag's offset at its last update.
    pub drag_offset: Cell<(f64, f64)>,
    pub presented_frames: Cell<u64>,
    pub gpu_lost_handlers: RefCell<Vec<StudioHandler>>,
    pub studio_failed_handlers: RefCell<Vec<StudioHandler>>,
    pub fallback_handlers: RefCell<Vec<StudioHandler>>,
    pub gpu_lost_announced: Cell<bool>,
}

#[glib::object_subclass]
impl ObjectSubclass for StudioView {
    const NAME: &'static str = "CadmiumStudioView";
    type Type = super::StudioView;
    type ParentType = gtk::Widget;
}

impl ObjectImpl for StudioView {
    fn constructed(&self) {
        self.parent_constructed();
        self.presenter
            .replace(FramePresenter::showing(PresentSource::Studio));
        let view = self.obj();
        view.set_hexpand(true);
        view.set_vexpand(true);
        view.set_size_request(STUDIO_MIN_WIDTH, STUDIO_MIN_HEIGHT);
        view.add_css_class(STUDIO_FRAME_CLASS);
        view.set_overflow(gtk::Overflow::Hidden);
        view.set_cursor_from_name(Some(STUDIO_CURSOR));
        view.connect_scale_factor_notify(super::StudioView::follow_scale_change);
    }
}

impl WidgetImpl for StudioView {
    fn realize(&self) {
        self.parent_realize();
        let view = self.obj();
        self.scale_watch
            .start(&*view, super::StudioView::follow_scale_change);
        view.queue_present();
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
        self.obj().queue_present();
    }

    fn snapshot(&self, snapshot: &gtk::Snapshot) {
        if let Some(texture) = self.texture.borrow().as_ref() {
            append_studio_image(snapshot, texture, self.obj().device_scale());
        }
    }
}
