use std::cell::{Cell, OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use cadmium_engine::canvas::{CanvasSpec, Primer, Weave};
use cadmium_engine::pigments::PigmentDb;
use gtk::glib;

use crate::swatch::Swatch;

mod build;
mod format;
mod imp;
mod selection;
mod signals;

use selection::PigmentSelection;

const CONTENT_WIDTH: i32 = 560;

const PRIMERS: [(Primer, &str); 2] = [
    (Primer::WhiteGesso, "White gesso"),
    (Primer::SiennaImprimatura, "Sienna imprimatura"),
];

const WEAVES: [(Weave, &str); 3] = [
    (Weave::Linen, "Linen"),
    (Weave::Cotton, "Cotton"),
    (Weave::Smooth, "Smooth"),
];

struct Widgets {
    format_row: adw::ComboRow,
    width_row: adw::SpinRow,
    height_row: adw::SpinRow,
    primer_row: adw::ComboRow,
    primer_swatch: Swatch,
    weave_row: adw::ComboRow,
    counter: gtk::Label,
    chips: Vec<(&'static str, gtk::ToggleButton)>,
    create_button: gtk::Button,
    primer_colours: Vec<[u8; 3]>,
}

type CreateHandler = Box<dyn Fn(&NewPaintingDialog, CanvasSpec)>;

glib::wrapper! {
    pub struct NewPaintingDialog(ObjectSubclass<imp::NewPaintingDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl NewPaintingDialog {
    pub fn new(db: &PigmentDb, initial: &CanvasSpec) -> Self {
        let dialog: NewPaintingDialog = glib::Object::builder()
            .property("title", "New Painting")
            .property("content-width", CONTENT_WIDTH)
            .build();
        dialog
            .imp()
            .selection
            .replace(Some(PigmentSelection::new(db, &initial.pigment_slots)));
        dialog.build(db, initial);
        dialog
    }

    pub fn connect_create<F: Fn(&NewPaintingDialog, CanvasSpec) + 'static>(&self, handler: F) {
        self.imp()
            .create_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn canvas_spec(&self) -> CanvasSpec {
        let widgets = self.widgets();
        widgets.width_row.update();
        widgets.height_row.update();
        CanvasSpec {
            width: widgets.width_row.value() as u32,
            height: widgets.height_row.value() as u32,
            primer: PRIMERS
                .get(widgets.primer_row.selected() as usize)
                .map_or(Primer::WhiteGesso, |(primer, _)| *primer),
            weave: WEAVES
                .get(widgets.weave_row.selected() as usize)
                .map_or(Weave::Linen, |(weave, _)| *weave),
            pigment_slots: self.with_selection(PigmentSelection::slots),
        }
    }

    fn widgets(&self) -> &Widgets {
        self.imp()
            .widgets
            .get()
            .expect("new painting dialog widgets are built in new")
    }

    fn with_selection<T>(&self, read: impl FnOnce(&PigmentSelection) -> T) -> T {
        read(
            self.imp()
                .selection
                .borrow()
                .as_ref()
                .expect("new painting dialog selection is set in new"),
        )
    }

    fn with_selection_mut<T>(&self, change: impl FnOnce(&mut PigmentSelection) -> T) -> T {
        change(
            self.imp()
                .selection
                .borrow_mut()
                .as_mut()
                .expect("new painting dialog selection is set in new"),
        )
    }

    fn create(&self) {
        let spec = self.canvas_spec();
        for handler in self.imp().create_handlers.borrow().iter() {
            handler(self, spec.clone());
        }
    }
}
