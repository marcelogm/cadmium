use super::*;

#[derive(Default)]
pub struct SheetView {
    pub state: OnceCell<SharedState>,
    pub presenter: RefCell<FramePresenter>,
    pub texture: RefCell<Option<gdk::Texture>>,
    pub needs_present: Cell<bool>,
    pub tick: RefCell<Option<gtk::TickCallbackId>>,
    pub scale_watch: SurfaceScaleWatch,
    /// Where the pointer is over the sheet; `None` off it.
    pub hover: Cell<Option<(f64, f64)>>,
    pub released_handlers: RefCell<Vec<Handler<super::SheetView>>>,
    pub colour_picked_handlers: RefCell<Vec<PickHandler>>,
    pub presented_handlers: RefCell<Vec<Handler<super::SheetView>>>,
    pub fallback_handlers: RefCell<Vec<Handler<super::SheetView>>>,
    pub gpu_lost_handlers: RefCell<Vec<Handler<super::SheetView>>>,
    pub gpu_lost_announced: Cell<bool>,
}

#[glib::object_subclass]
impl ObjectSubclass for SheetView {
    const NAME: &'static str = "CadmiumSheetView";
    type Type = super::SheetView;
    type ParentType = gtk::Widget;
}

impl ObjectImpl for SheetView {
    fn constructed(&self) {
        self.parent_constructed();
        self.presenter
            .replace(FramePresenter::showing(PresentSource::Sheet));
        let view = self.obj();
        view.set_hexpand(true);
        view.set_vexpand(true);
        view.add_css_class(SHEET_FRAME_CLASS);
        view.set_overflow(gtk::Overflow::Hidden);
        view.set_cursor_from_name(Some(SHEET_CURSOR));
        view.connect_scale_factor_notify(super::SheetView::queue_present);
    }
}

impl WidgetImpl for SheetView {
    fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
        let minimum = match orientation {
            gtk::Orientation::Horizontal => SHEET_MIN_SIZE.0,
            _ => SHEET_MIN_SIZE.1,
        };
        (minimum, minimum, -1, -1)
    }

    fn realize(&self) {
        self.parent_realize();
        let view = self.obj();
        self.scale_watch
            .start(&*view, super::SheetView::queue_present);
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
        let view = self.obj();
        if let Some(texture) = self.texture.borrow().as_ref() {
            append_sheet_image(snapshot, texture, device_scale(&*view));
        }
        if let Some((x, y)) = self.hover.get()
            && let Some(outline) = view.cursor_outline()
            && outline.is_visible()
        {
            outline.append_to(snapshot, (x as f32, y as f32));
        }
    }
}
