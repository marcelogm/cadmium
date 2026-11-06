//! A brush's tip in a rack row: the front view in ink, the ferrule above the hairs.
//!
//! Lines only, in the widget's foreground colour so it reads in light and dark themes: the
//! ferrule dimmed, the hair bundle outlined over a faint fill. No handle and no guide lines
//! at this size.

use adw::prelude::*;
use cadmium_studio::TipView;
use gtk::{cairo, gdk};

use crate::ink::{contain, trace};

const ICON_SIZE: i32 = 30;
const PADDING: f64 = 2.0;
const LINE_WIDTH: f64 = 1.0;
const FERRULE_ALPHA: f32 = 0.7;
const HAIR_FILL_ALPHA: f32 = 0.14;

pub(super) fn tip_icon(view: TipView) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(ICON_SIZE)
        .content_height(ICON_SIZE)
        .valign(gtk::Align::Center)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    area.set_draw_func(move |area, context, width, height| {
        draw(
            context,
            &view,
            &area.color(),
            [width, height],
            pixel_scale(area),
        );
    });
    area
}

/// Device pixels per logical one where `widget` shows, fractional scaling included.
fn pixel_scale(widget: &impl IsA<gtk::Widget>) -> f64 {
    widget
        .native()
        .and_then(|native| native.surface())
        .map_or_else(
            || f64::from(widget.scale_factor()),
            |surface| surface.scale(),
        )
}

/// Draws `view` fitted into `size`, its mouth line and axis on whole device pixels at
/// `pixel_scale`.
fn draw(
    context: &cairo::Context,
    view: &TipView,
    ink: &gdk::RGBA,
    size: [i32; 2],
    pixel_scale: f64,
) {
    let [min, max] = view.bounds().map(|corner| corner.map(f64::from));
    let fit = contain(min, max, size.map(f64::from), PADDING).snapped(pixel_scale, LINE_WIDTH);
    context.set_line_width(LINE_WIDTH);
    context.set_line_join(cairo::LineJoin::Round);
    trace(context, &fit, &view.ferrule.outline);
    set_ink(context, ink, FERRULE_ALPHA);
    let _ = context.stroke();
    trace(context, &fit, &view.outline);
    set_ink(context, ink, HAIR_FILL_ALPHA);
    let _ = context.fill_preserve();
    set_ink(context, ink, 1.0);
    let _ = context.stroke();
}

fn set_ink(context: &cairo::Context, ink: &gdk::RGBA, alpha: f32) {
    context.set_source_rgba(
        f64::from(ink.red()),
        f64::from(ink.green()),
        f64::from(ink.blue()),
        f64::from(ink.alpha() * alpha),
    );
}

#[cfg(test)]
mod tests;
