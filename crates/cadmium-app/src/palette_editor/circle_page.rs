//! The Circle page: the big editable wheel, its hint and the Darken slider.

use adw::prelude::*;
use cadmium_engine::palette::{MAX_DARKEN, circle_axes};

use super::PaletteEditor;
use super::presentation::CIRCLE_HINT;
use super::snapshot::Snapshot;
use crate::brush_panel::wheel_is_stale;
use crate::colour_wheel::{ColourWheel, WheelPaint, wheel_image};
use crate::state::PaletteMode;

const WHEEL_SIZE_PX: i32 = 400;
const PAGE_MARGIN: i32 = 18;
const PAGE_SPACING: i32 = 18;
const DARKEN_TITLE: &str = "Darken";
const DARKEN_TOOLTIP: &str = "Adds black to the colours chosen on the circle";
const DARKEN_WIDTH: i32 = 200;
const DARKEN_SPACING: i32 = 12;
const DARKEN_STEP: f64 = 0.01;
const DARKEN_PAGE: f64 = 0.1;

pub(super) struct CirclePage {
    pub(super) root: gtk::Box,
    pub(super) wheel: ColourWheel,
    pub(super) darken: gtk::Scale,
}

pub(super) fn circle_page() -> CirclePage {
    let hint = gtk::Label::builder()
        .label(CIRCLE_HINT)
        .wrap(true)
        .xalign(0.0)
        .hexpand(true)
        .css_classes(["dim-label"])
        .build();
    let darken = darken_scale();
    let top = gtk::Box::builder().spacing(DARKEN_SPACING).build();
    top.append(&hint);
    top.append(&gtk::Label::new(Some(DARKEN_TITLE)));
    top.append(&darken);
    let wheel = ColourWheel::new(WHEEL_SIZE_PX);
    wheel.set_editable(true);
    wheel.widget().set_vexpand(true);
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(PAGE_SPACING)
        .margin_top(PAGE_MARGIN)
        .margin_bottom(PAGE_MARGIN)
        .margin_start(PAGE_MARGIN)
        .margin_end(PAGE_MARGIN)
        .build();
    root.append(&top);
    root.append(wheel.widget());
    CirclePage {
        root,
        wheel,
        darken,
    }
}

fn darken_scale() -> gtk::Scale {
    let adjustment = gtk::Adjustment::new(
        0.0,
        0.0,
        f64::from(MAX_DARKEN),
        DARKEN_STEP,
        DARKEN_PAGE,
        0.0,
    );
    let darken = gtk::Scale::builder()
        .adjustment(&adjustment)
        .draw_value(false)
        .width_request(DARKEN_WIDTH)
        .tooltip_text(DARKEN_TOOLTIP)
        .build();
    darken.update_property(&[gtk::accessible::Property::Label(DARKEN_TITLE)]);
    darken
}

impl PaletteEditor {
    /// Shows the darkening and, while the circle shows and what it is mixed from changed,
    /// mixes the wheel again.
    pub(super) fn show_circle(&self, snapshot: &Snapshot) {
        let page = &self.circle;
        page.darken.set_sensitive(snapshot.can_darken);
        page.darken.set_value(f64::from(snapshot.edit.darken));
        let wanted = &snapshot.wheel_key;
        let shows = snapshot.edit.mode == PaletteMode::Circle;
        if !wheel_is_stale(shows, self.shown.wheel.borrow().as_ref(), wanted) {
            return;
        }
        let state = self.state.borrow();
        let image = wheel_image(&WheelPaint {
            db: &state.painting.session.db,
            slots: &wanted.slots,
            palette: &wanted.palette,
            axes: circle_axes(&wanted.slots),
            darken: f32::from_bits(wanted.darken_bits),
        });
        drop(state);
        page.wheel.show(image);
        self.shown.wheel.replace(Some(wanted.clone()));
    }
}
