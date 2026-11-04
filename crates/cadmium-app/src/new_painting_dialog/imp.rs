use super::*;

#[derive(Default)]
pub struct NewPaintingDialog {
    pub(super) widgets: OnceCell<Widgets>,
    pub selection: RefCell<Option<PigmentSelection>>,
    pub syncing_size: Cell<bool>,
    pub counter_flash: RefCell<Option<glib::SourceId>>,
    pub create_handlers: RefCell<Vec<CreateHandler>>,
}

#[glib::object_subclass]
impl ObjectSubclass for NewPaintingDialog {
    const NAME: &'static str = "CadmiumNewPaintingDialog";
    type Type = super::NewPaintingDialog;
    type ParentType = adw::Dialog;
}

impl ObjectImpl for NewPaintingDialog {
    fn dispose(&self) {
        if let Some(source) = self.counter_flash.take() {
            source.remove();
        }
        self.create_handlers.borrow_mut().clear();
    }
}

impl WidgetImpl for NewPaintingDialog {}
impl AdwDialogImpl for NewPaintingDialog {}
