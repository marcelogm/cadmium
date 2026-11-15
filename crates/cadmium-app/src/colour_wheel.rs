//! The colour wheel: the painting's circle of colours, whiter towards the centre.
//!
//! The picture is a coarse grid Cairo scales up, so redrawing it while a slider moves is cheap.

use std::cell::{Cell, RefCell};
use std::f64::consts::TAU;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{cairo, gdk, glib};

use cadmium_engine::palette::ColourRef;

use crate::LOG_DOMAIN;

mod dragging;
mod geometry;
mod image;

pub(crate) use dragging::colour_ref_text;
use geometry::rim_centre;
use image::WheelImage;
use image::{ARGB32_STRIDE, WHEEL_CELLS};
pub(crate) use image::{WheelPaint, wheel_image};

const NEUTRAL_GREY: f64 = 225.0 / 255.0;
const CHANNEL_MAX: f64 = 255.0;
const RIM_DOT_RADIUS_PX: f64 = 5.0;
const EDITABLE_RIM_DOT_RADIUS_PX: f64 = 8.0;
const RIM_DOT_RING_WIDTH_PX: f64 = 1.5;
const RIM_DOT_SHADE_WIDTH_PX: f64 = 1.0;
const RIM_DOT_SHADE_ALPHA: f64 = 0.35;

/// Whether a pick is the press that starts a gesture on the wheel or the pointer moving after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WheelPick {
    Press,
    Drag,
}

/// A point of the wheel, as its listener hears it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PickedPoint {
    pub(crate) turn: f32,
    pub(crate) radius: f32,
    pub(crate) pick: WheelPick,
    /// The modifiers held at the press that started the gesture, wherever that press fell.
    pub(crate) modifiers: gdk::ModifierType,
}

type Picked = Rc<dyn Fn(PickedPoint)>;
type Dropped = Rc<dyn Fn(ColourRef, f32)>;
type Dragged = Rc<dyn Fn(usize, f32) -> usize>;
type Removed = Rc<dyn Fn(usize)>;

#[derive(Default)]
struct Shown {
    surface: Option<cairo::ImageSurface>,
    rim: Vec<[u8; 3]>,
}

impl Shown {
    fn of(image: WheelImage) -> Shown {
        Shown {
            surface: image.has_colour().then(|| surface_of(&image)).flatten(),
            rim: image.rim,
        }
    }
}

#[derive(Default)]
struct Listeners {
    picked: RefCell<Option<Picked>>,
    dropped: RefCell<Option<Dropped>>,
    dragged: RefCell<Option<Dragged>>,
    removed: RefCell<Option<Removed>>,
}

/// What the wheel's drawing and its controllers share.
#[derive(Default)]
struct Wheel {
    shown: RefCell<Shown>,
    editable: Cell<bool>,
    listeners: Listeners,
}

pub(crate) struct ColourWheel {
    area: gtk::DrawingArea,
    wheel: Rc<Wheel>,
}

impl ColourWheel {
    pub(crate) fn new(size_px: i32) -> ColourWheel {
        let area = gtk::DrawingArea::builder()
            .content_width(size_px)
            .content_height(size_px)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        let wheel: Rc<Wheel> = Rc::default();
        let drawn = Rc::clone(&wheel);
        area.set_draw_func(move |_, context, width, height| {
            let side_px = side_px(width, height);
            draw(
                &drawn.shown.borrow(),
                context,
                side_px,
                drawn.editable.get(),
            );
        });
        dragging::attach(&area, &wheel);
        ColourWheel { area, wheel }
    }

    pub(crate) fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    pub(crate) fn show(&self, image: WheelImage) {
        self.wheel.shown.replace(Shown::of(image));
        self.area.queue_draw();
    }

    /// An editable wheel draws bigger rim dots, lets them be dragged round or off it, and takes
    /// colours dropped onto it; a wheel that is not ignores all three.
    pub(crate) fn set_editable(&self, editable: bool) {
        self.wheel.editable.set(editable);
        self.area.queue_draw();
    }

    /// `picked` hears every press and every drag inside the disc, but for a drag that starts on
    /// a rim dot of an editable wheel.
    pub(crate) fn connect_picked(&self, picked: impl Fn(PickedPoint) + 'static) {
        self.wheel.listeners.picked.replace(Some(Rc::new(picked)));
    }

    /// `dropped` hears a colour dropped onto an editable wheel and the turn where it fell.
    pub(crate) fn connect_colour_dropped(&self, dropped: impl Fn(ColourRef, f32) + 'static) {
        self.wheel.listeners.dropped.replace(Some(Rc::new(dropped)));
    }

    /// `dragged` hears every move of a grabbed rim dot, as its position and the pointer's turn,
    /// and answers the position the dot has moved to.
    pub(crate) fn connect_rim_dragged(&self, dragged: impl Fn(usize, f32) -> usize + 'static) {
        self.wheel.listeners.dragged.replace(Some(Rc::new(dragged)));
    }

    /// `removed` hears the position of a rim dot let go well past the edge of the wheel.
    pub(crate) fn connect_rim_removed(&self, removed: impl Fn(usize) + 'static) {
        self.wheel.listeners.removed.replace(Some(Rc::new(removed)));
    }
}

fn side_px(width: i32, height: i32) -> f64 {
    f64::from(width.min(height))
}

fn surface_of(image: &WheelImage) -> Option<cairo::ImageSurface> {
    let cells = i32::try_from(WHEEL_CELLS).ok()?;
    let stride = i32::try_from(ARGB32_STRIDE).ok()?;
    cairo::ImageSurface::create_for_data(
        image.argb32(),
        cairo::Format::ARgb32,
        cells,
        cells,
        stride,
    )
    .ok()
}

fn draw(shown: &Shown, context: &cairo::Context, side_px: f64, editable: bool) {
    if side_px <= 0.0 {
        return;
    }
    let half_px = side_px / 2.0;
    context.new_sub_path();
    context.arc(half_px, half_px, half_px, 0.0, TAU);
    context.set_source_rgb(NEUTRAL_GREY, NEUTRAL_GREY, NEUTRAL_GREY);
    if let Err(error) = context.fill_preserve() {
        glib::g_warning!(LOG_DOMAIN, "could not paint the colour wheel: {error}");
    }
    if let Some(surface) = &shown.surface {
        paint_cells(context, surface, side_px);
    }
    context.new_path();
    let dot_radius_px = if editable {
        EDITABLE_RIM_DOT_RADIUS_PX
    } else {
        RIM_DOT_RADIUS_PX
    };
    for (position, colour) in shown.rim.iter().enumerate() {
        let centre = rim_centre(position, shown.rim.len(), side_px);
        draw_rim_dot(context, centre, dot_radius_px, *colour);
    }
}

fn paint_cells(context: &cairo::Context, surface: &cairo::ImageSurface, side_px: f64) {
    let scale = side_px / WHEEL_CELLS as f64;
    let painted = context.save().and_then(|()| {
        context.clip_preserve();
        context.scale(scale, scale);
        context.set_source_surface(surface, 0.0, 0.0)?;
        context.source().set_filter(cairo::Filter::Bilinear);
        context.source().set_extend(cairo::Extend::Pad);
        context.paint()
    });
    if let Err(error) = painted.and_then(|()| context.restore()) {
        glib::g_warning!(LOG_DOMAIN, "could not paint the colour wheel: {error}");
    }
}

fn draw_rim_dot(context: &cairo::Context, (x, y): (f64, f64), radius_px: f64, colour: [u8; 3]) {
    let [red, green, blue] = colour.map(|channel| f64::from(channel) / CHANNEL_MAX);
    let ring_radius_px = radius_px + RIM_DOT_RING_WIDTH_PX / 2.0;
    let shade_radius_px = radius_px + RIM_DOT_RING_WIDTH_PX + RIM_DOT_SHADE_WIDTH_PX / 2.0;
    context.new_sub_path();
    context.arc(x, y, radius_px, 0.0, TAU);
    context.set_source_rgb(red, green, blue);
    let _ = context.fill();
    context.arc(x, y, ring_radius_px, 0.0, TAU);
    context.set_source_rgb(1.0, 1.0, 1.0);
    context.set_line_width(RIM_DOT_RING_WIDTH_PX);
    let _ = context.stroke();
    context.arc(x, y, shade_radius_px, 0.0, TAU);
    context.set_source_rgba(0.0, 0.0, 0.0, RIM_DOT_SHADE_ALPHA);
    context.set_line_width(RIM_DOT_SHADE_WIDTH_PX);
    let _ = context.stroke();
}

#[cfg(test)]
mod tests;
