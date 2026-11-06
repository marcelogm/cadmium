//! A rack row's stroke: the brush's thumbnail scaled into a fixed size, painted with Cairo.
//!
//! A fixed-size drawing area rather than a `gtk::Picture`, whose natural size would follow
//! the texture and take the labels' width or stretch the row.

use std::cell::RefCell;
use std::f64::consts::{FRAC_PI_2, PI};
use std::rc::Rc;

use adw::prelude::*;
use cadmium_studio::THUMBNAIL_SIZE;
use gtk::{cairo, gdk, glib};

use super::fit::contain;
use crate::LOG_DOMAIN;

const STROKE_HEIGHT: u32 = 46;
/// The thumbnail at `STROKE_HEIGHT`, keeping its aspect: 153 × 46 for 240 × 72, the width
/// truncated from 153.33; `contain` letterboxes the fraction.
pub(super) const STROKE_SIZE: (i32, i32) = (
    (STROKE_HEIGHT * THUMBNAIL_SIZE.0 / THUMBNAIL_SIZE.1) as i32,
    STROKE_HEIGHT as i32,
);
const CORNER_RADIUS: f64 = 5.0;

pub(super) struct StrokeView {
    area: gtk::DrawingArea,
    surface: Rc<RefCell<Option<cairo::ImageSurface>>>,
}

impl StrokeView {
    pub(super) fn new() -> StrokeView {
        let area = gtk::DrawingArea::builder()
            .content_width(STROKE_SIZE.0)
            .content_height(STROKE_SIZE.1)
            .valign(gtk::Align::Center)
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        let surface: Rc<RefCell<Option<cairo::ImageSurface>>> = Rc::default();
        let shown = Rc::clone(&surface);
        area.set_draw_func(move |_, context, width, height| {
            draw(&shown, context, [f64::from(width), f64::from(height)]);
        });
        StrokeView { area, surface }
    }

    pub(super) fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    /// Shows `thumbnail`, or nothing until one is ready.
    pub(super) fn show(&self, thumbnail: Option<&gdk::Texture>) {
        self.surface.replace(thumbnail.and_then(surface_of));
        self.area.queue_draw();
    }
}

/// Copies `texture` into a Cairo surface: GDK downloads in Cairo's ARGB32 layout.
fn surface_of(texture: &gdk::Texture) -> Option<cairo::ImageSurface> {
    let (width, height) = (texture.width(), texture.height());
    let stride = cairo::Format::ARgb32
        .stride_for_width(u32::try_from(width).ok()?)
        .ok()?;
    let mut data = vec![0; usize::try_from(stride).ok()? * usize::try_from(height).ok()?];
    texture.download(&mut data, usize::try_from(stride).ok()?);
    cairo::ImageSurface::create_for_data(data, cairo::Format::ARgb32, width, height, stride).ok()
}

fn draw(shown: &RefCell<Option<cairo::ImageSurface>>, context: &cairo::Context, size: [f64; 2]) {
    if let Some(surface) = shown.borrow().as_ref() {
        paint(context, surface, size);
    }
}

fn paint(context: &cairo::Context, surface: &cairo::ImageSurface, size: [f64; 2]) {
    let picture = [f64::from(surface.width()), f64::from(surface.height())];
    let fit = contain([0.0; 2], picture, size, 0.0);
    let (x, y) = fit.place([0.0, 0.0]);
    rounded_rect(context, [x, y], picture.map(|side| side * fit.scale));
    context.clip();
    context.translate(x, y);
    context.scale(fit.scale, fit.scale);
    if context.set_source_surface(surface, 0.0, 0.0).is_ok() {
        context.source().set_filter(cairo::Filter::Good);
        if let Err(error) = context.paint() {
            glib::g_warning!(LOG_DOMAIN, "could not paint a brush stroke: {error}");
        }
    }
}

fn rounded_rect(context: &cairo::Context, [x, y]: [f64; 2], [width, height]: [f64; 2]) {
    let radius = CORNER_RADIUS.min(width / 2.0).min(height / 2.0);
    let (left, top) = (x + radius, y + radius);
    let (right, bottom) = (x + width - radius, y + height - radius);
    context.new_sub_path();
    context.arc(right, top, radius, -FRAC_PI_2, 0.0);
    context.arc(right, bottom, radius, 0.0, FRAC_PI_2);
    context.arc(left, bottom, radius, FRAC_PI_2, PI);
    context.arc(left, top, radius, PI, 1.5 * PI);
    context.close_path();
}
