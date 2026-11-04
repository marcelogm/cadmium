use super::*;

#[derive(Default)]
pub struct Swatch {
    pub colour: Cell<[u8; 3]>,
    pub right_half_colour: Cell<Option<[u8; 3]>>,
    pub selected: Cell<bool>,
    pub shape: Cell<SwatchShape>,
    pub accent_colour_handler: RefCell<Option<glib::SignalHandlerId>>,
}

#[glib::object_subclass]
impl ObjectSubclass for Swatch {
    const NAME: &'static str = "CadmiumSwatch";
    type Type = super::Swatch;
    type ParentType = gtk::DrawingArea;
}

impl ObjectImpl for Swatch {
    fn constructed(&self) {
        self.parent_constructed();
        let swatch = self.obj();
        swatch.set_draw_func(draw_swatch);
        let handler = adw::StyleManager::default().connect_accent_color_notify(glib::clone!(
            #[weak]
            swatch,
            move |_| swatch.queue_draw()
        ));
        self.accent_colour_handler.replace(Some(handler));
    }

    fn dispose(&self) {
        if let Some(handler) = self.accent_colour_handler.take() {
            adw::StyleManager::default().disconnect(handler);
        }
    }
}

impl WidgetImpl for Swatch {}
impl DrawingAreaImpl for Swatch {}

fn draw_swatch(area: &gtk::DrawingArea, context: &cairo::Context, width: i32, height: i32) {
    if let Some(swatch) = area.downcast_ref::<super::Swatch>() {
        swatch.draw(context, width as f64, height as f64);
    }
}
