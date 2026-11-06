use std::cell::{Cell, OnceCell, RefCell};

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene};

use super::{CURSOR, DESCRIPTION, GRID_ALPHA, Link, append_sheet};
use crate::brush_editor::paper::{draw_grid, draw_grid_lines};
use crate::handlers::Handler;
use crate::presenter::FramePresenter;
use crate::surface_scale::{SurfaceScaleWatch, device_scale};

#[derive(Default)]
pub struct TestSheetView {
    pub(super) link: OnceCell<Link>,
    pub(super) presenter: RefCell<FramePresenter>,
    pub(super) texture: RefCell<Option<gdk::Texture>>,
    pub(super) needs_present: Cell<bool>,
    pub(super) tick: RefCell<Option<gtk::TickCallbackId>>,
    pub(super) scale_watch: SurfaceScaleWatch,
    /// A stroke is under way on the sheet.
    pub(super) stroking: Cell<bool>,
    /// The device painting the stroke under way, when known.
    pub(super) stroking_device: RefCell<Option<gdk::Device>>,
    pub(super) gpu_lost_announced: Cell<bool>,
    pub(super) gpu_lost_handlers: RefCell<Vec<Handler<super::TestSheetView>>>,
    pub(super) fallback_handlers: RefCell<Vec<Handler<super::TestSheetView>>>,
}

#[glib::object_subclass]
impl ObjectSubclass for TestSheetView {
    const NAME: &'static str = "CadmiumTestSheetView";
    type Type = super::TestSheetView;
    type ParentType = gtk::Widget;
}

impl ObjectImpl for TestSheetView {
    fn constructed(&self) {
        self.parent_constructed();
        let view = self.obj();
        view.set_hexpand(true);
        view.set_vexpand(true);
        view.add_css_class("card");
        view.set_overflow(gtk::Overflow::Hidden);
        view.set_cursor_from_name(Some(CURSOR));
        view.update_property(&[gtk::accessible::Property::Label(DESCRIPTION)]);
        view.connect_scale_factor_notify(super::TestSheetView::queue_present);
    }
}

impl WidgetImpl for TestSheetView {
    fn realize(&self) {
        self.parent_realize();
        let view = self.obj();
        self.scale_watch
            .start(&*view, super::TestSheetView::queue_present);
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

    /// The presented canvas with the grid's faint lines over it; plain gridded paper until
    /// the first frame.
    fn snapshot(&self, snapshot: &gtk::Snapshot) {
        let view = self.obj();
        let size = [f64::from(view.width()), f64::from(view.height())];
        let bounds = graphene::Rect::new(0.0, 0.0, size[0] as f32, size[1] as f32);
        let texture = self.texture.borrow();
        let Some(texture) = texture.as_ref() else {
            draw_grid(&snapshot.append_cairo(&bounds), size);
            return;
        };
        append_sheet(snapshot, texture, device_scale(&*view));
        draw_grid_lines(&snapshot.append_cairo(&bounds), size, GRID_ALPHA);
    }
}
