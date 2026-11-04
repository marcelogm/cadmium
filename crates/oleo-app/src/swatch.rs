use std::cell::{Cell, RefCell};
use std::f64::consts::TAU;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{cairo, gdk, glib};

const BORDER_WIDTH: f64 = 1.0;
const BORDER_ALPHA: f32 = 0.18;
const RING_WIDTH: f64 = 2.0;
const RING_GAP: f64 = 1.5;
const CORNER_RADIUS: f64 = 4.0;
const CHANNEL_MAX: f64 = 255.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwatchShape {
    #[default]
    Circle,
    RoundedRect,
}

mod imp {
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
        const NAME: &'static str = "OleoSwatch";
        type Type = super::Swatch;
        type ParentType = gtk::DrawingArea;
    }

    impl ObjectImpl for Swatch {
        fn constructed(&self) {
            self.parent_constructed();
            let swatch = self.obj();
            swatch.set_draw_func(|area, context, width, height| {
                if let Some(swatch) = area.downcast_ref::<super::Swatch>() {
                    swatch.draw(context, width as f64, height as f64);
                }
            });
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
}

glib::wrapper! {
    pub struct Swatch(ObjectSubclass<imp::Swatch>)
        @extends gtk::DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Swatch {
    pub fn new(colour: [u8; 3], shape: SwatchShape, width: i32, height: i32) -> Self {
        let swatch: Swatch = glib::Object::builder()
            .property("content-width", width)
            .property("content-height", height)
            .property("valign", gtk::Align::Center)
            .property("halign", gtk::Align::Center)
            .build();
        swatch.imp().colour.set(colour);
        swatch.imp().shape.set(shape);
        swatch
    }

    pub fn colour(&self) -> [u8; 3] {
        self.imp().colour.get()
    }

    pub fn set_colour(&self, colour: [u8; 3]) {
        if self.imp().colour.replace(colour) != colour {
            self.queue_draw();
        }
    }

    pub fn set_right_half_colour(&self, colour: Option<[u8; 3]>) {
        if self.imp().right_half_colour.replace(colour) != colour {
            self.queue_draw();
        }
    }

    pub fn is_selected(&self) -> bool {
        self.imp().selected.get()
    }

    pub fn set_selected(&self, selected: bool) {
        if self.imp().selected.replace(selected) != selected {
            self.queue_draw();
        }
    }

    fn draw(&self, context: &cairo::Context, width: f64, height: f64) {
        let shape = self.imp().shape.get();
        let selected = self.is_selected();
        let inset = if selected { RING_WIDTH + RING_GAP } else { 0.0 };
        if selected {
            let accent = adw::StyleManager::default().accent_color_rgba();
            trace_shape(context, shape, width, height, RING_WIDTH / 2.0);
            set_source(context, &accent);
            context.set_line_width(RING_WIDTH);
            let _ = context.stroke();
        }
        let [red, green, blue] = self.colour().map(channel_fraction);
        trace_shape(context, shape, width, height, inset + BORDER_WIDTH / 2.0);
        context.set_source_rgb(red, green, blue);
        let _ = context.fill_preserve();
        if let Some(right) = self.imp().right_half_colour.get() {
            let [red, green, blue] = right.map(channel_fraction);
            let _ = context.save();
            context.clip();
            context.rectangle(width / 2.0, 0.0, width / 2.0, height);
            context.set_source_rgb(red, green, blue);
            let _ = context.fill();
            let _ = context.restore();
            trace_shape(context, shape, width, height, inset + BORDER_WIDTH / 2.0);
        }
        let border = self.color().with_alpha(BORDER_ALPHA);
        set_source(context, &border);
        context.set_line_width(BORDER_WIDTH);
        let _ = context.stroke();
    }
}

fn channel_fraction(channel: u8) -> f64 {
    f64::from(channel) / CHANNEL_MAX
}

fn set_source(context: &cairo::Context, colour: &gdk::RGBA) {
    context.set_source_rgba(
        colour.red() as f64,
        colour.green() as f64,
        colour.blue() as f64,
        colour.alpha() as f64,
    );
}

fn trace_shape(context: &cairo::Context, shape: SwatchShape, width: f64, height: f64, inset: f64) {
    let (left, top) = (inset, inset);
    let (right, bottom) = (width - inset, height - inset);
    if right <= left || bottom <= top {
        return;
    }
    match shape {
        SwatchShape::Circle => {
            let radius = (right - left).min(bottom - top) / 2.0;
            context.new_sub_path();
            context.arc(width / 2.0, height / 2.0, radius, 0.0, TAU);
        }
        SwatchShape::RoundedRect => {
            let radius = (CORNER_RADIUS - inset)
                .max(1.0)
                .min((right - left) / 2.0)
                .min((bottom - top) / 2.0);
            let quarter = TAU / 4.0;
            context.new_sub_path();
            context.arc(right - radius, top + radius, radius, -quarter, 0.0);
            context.arc(right - radius, bottom - radius, radius, 0.0, quarter);
            context.arc(
                left + radius,
                bottom - radius,
                radius,
                quarter,
                2.0 * quarter,
            );
            context.arc(
                left + radius,
                top + radius,
                radius,
                2.0 * quarter,
                3.0 * quarter,
            );
            context.close_path();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style_manager_has_accent_colour_handler() -> bool {
        let style_manager = adw::StyleManager::default();
        let notify = glib::subclass::SignalId::lookup("notify", style_manager.type_())
            .expect("notify signal");
        glib::signal::signal_has_handler_pending(
            &style_manager,
            notify,
            Some(glib::Quark::from_str("accent-color")),
            true,
        )
    }

    #[test]
    fn dropped_swatches_leave_no_accent_colour_handler_behind() {
        adw::init().expect("libadwaita initialises");
        let before = style_manager_has_accent_colour_handler();
        let swatches: Vec<_> = (0..64)
            .map(|_| Swatch::new([0, 0, 0], SwatchShape::Circle, 16, 16))
            .collect();
        assert!(style_manager_has_accent_colour_handler());
        let finalized: Vec<_> = swatches.iter().map(|swatch| swatch.downgrade()).collect();
        drop(swatches);
        assert!(finalized.iter().all(|swatch| swatch.upgrade().is_none()));
        assert_eq!(style_manager_has_accent_colour_handler(), before);
    }
}
