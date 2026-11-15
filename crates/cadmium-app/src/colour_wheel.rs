//! The colour wheel: the painting's circle of colours, whiter towards the centre.
//!
//! The picture is a coarse grid Cairo scales up, so redrawing it while a slider moves is cheap.

use std::cell::RefCell;
use std::f64::consts::TAU;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{cairo, gdk, glib};

use crate::LOG_DOMAIN;

mod geometry;
mod image;

use geometry::{rim_centre, turn_and_radius_at};
use image::WHEEL_CELLS;
use image::WheelImage;
pub(crate) use image::{WheelPaint, wheel_image};

const NEUTRAL_GREY: f64 = 225.0 / 255.0;
const CHANNEL_MAX: f64 = 255.0;
const RIM_DOT_RADIUS_PX: f64 = 5.0;
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
    pub(crate) modifiers: gdk::ModifierType,
}

type Picked = Rc<dyn Fn(PickedPoint)>;

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

pub(crate) struct ColourWheel {
    area: gtk::DrawingArea,
    shown: Rc<RefCell<Shown>>,
    picked: Rc<RefCell<Option<Picked>>>,
}

impl ColourWheel {
    pub(crate) fn new(size_px: i32) -> ColourWheel {
        let area = gtk::DrawingArea::builder()
            .content_width(size_px)
            .content_height(size_px)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        let shown: Rc<RefCell<Shown>> = Rc::default();
        let drawn = Rc::clone(&shown);
        area.set_draw_func(move |_, context, width, height| {
            draw(&drawn.borrow(), context, side_px(width, height));
        });
        let picked: Rc<RefCell<Option<Picked>>> = Rc::default();
        attach_drag(&area, &picked);
        ColourWheel {
            area,
            shown,
            picked,
        }
    }

    pub(crate) fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    pub(crate) fn show(&self, image: WheelImage) {
        self.shown.replace(Shown::of(image));
        self.area.queue_draw();
    }

    /// `picked` hears every press and every drag inside the disc.
    pub(crate) fn connect_picked(&self, picked: impl Fn(PickedPoint) + 'static) {
        self.picked.replace(Some(Rc::new(picked)));
    }
}

fn side_px(width: i32, height: i32) -> f64 {
    f64::from(width.min(height))
}

fn attach_drag(area: &gtk::DrawingArea, picked: &Rc<RefCell<Option<Picked>>>) {
    let gesture = gtk::GestureDrag::new();
    gesture.connect_drag_begin(glib::clone!(
        #[strong]
        picked,
        move |gesture, x, y| pick(gesture, &picked, WheelPick::Press, (x, y))
    ));
    gesture.connect_drag_update(glib::clone!(
        #[strong]
        picked,
        move |gesture, dx, dy| {
            if let Some((x, y)) = gesture.start_point() {
                pick(gesture, &picked, WheelPick::Drag, (x + dx, y + dy));
            }
        }
    ));
    area.add_controller(gesture);
}

fn pick(
    gesture: &gtk::GestureDrag,
    picked: &RefCell<Option<Picked>>,
    pick: WheelPick,
    (x, y): (f64, f64),
) {
    let Some(area) = gesture.widget() else {
        return;
    };
    let Some((turn, radius)) = turn_and_radius_at(x, y, side_px(area.width(), area.height()))
    else {
        return;
    };
    let listener = picked.borrow().clone();
    if let Some(listener) = listener {
        listener(PickedPoint {
            turn,
            radius,
            pick,
            modifiers: gesture.current_event_state(),
        });
    }
}

fn surface_of(image: &WheelImage) -> Option<cairo::ImageSurface> {
    let cells = i32::try_from(WHEEL_CELLS).ok()?;
    let stride = cairo::Format::ARgb32
        .stride_for_width(u32::try_from(WHEEL_CELLS).ok()?)
        .ok()?;
    let data = image.argb32(usize::try_from(stride).ok()?);
    cairo::ImageSurface::create_for_data(data, cairo::Format::ARgb32, cells, cells, stride).ok()
}

fn draw(shown: &Shown, context: &cairo::Context, side_px: f64) {
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
    for (position, colour) in shown.rim.iter().enumerate() {
        let centre = rim_centre(position, shown.rim.len(), side_px);
        draw_rim_dot(context, centre, *colour);
    }
}

fn paint_cells(context: &cairo::Context, surface: &cairo::ImageSurface, side_px: f64) {
    let scale = side_px / WHEEL_CELLS as f64;
    let painted = context.save().and_then(|()| {
        context.clip_preserve();
        context.scale(scale, scale);
        context.set_source_surface(surface, 0.0, 0.0)?;
        context.source().set_filter(cairo::Filter::Bilinear);
        context.paint()
    });
    if let Err(error) = painted.and_then(|()| context.restore()) {
        glib::g_warning!(LOG_DOMAIN, "could not paint the colour wheel: {error}");
    }
}

fn draw_rim_dot(context: &cairo::Context, (x, y): (f64, f64), colour: [u8; 3]) {
    let [red, green, blue] = colour.map(|channel| f64::from(channel) / CHANNEL_MAX);
    let ring_radius_px = RIM_DOT_RADIUS_PX + RIM_DOT_RING_WIDTH_PX / 2.0;
    let shade_radius_px = RIM_DOT_RADIUS_PX + RIM_DOT_RING_WIDTH_PX + RIM_DOT_SHADE_WIDTH_PX / 2.0;
    context.new_sub_path();
    context.arc(x, y, RIM_DOT_RADIUS_PX, 0.0, TAU);
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
