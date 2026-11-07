//! A small pad to try the curve on: each move leaves a line as wide as the shaped pressure.
//! Plain cairo lines, not the paint engine.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::input::samples::{is_primary_contact, motion_samples};
use crate::pressure_curve::PressureCurve;

/// The width of a line at full pressure, in widget px.
pub(super) const MAX_WIDTH: f64 = 24.0;
const PAD_SIZE: i32 = 200;
const MARK_ALPHA: f64 = 0.85;

/// One move of a stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Mark {
    pub(super) from: (f64, f64),
    pub(super) to: (f64, f64),
    pub(super) width: f64,
}

/// The marks on the pad and where the stroke under way last was.
#[derive(Debug, Default)]
pub(super) struct Scribble {
    marks: Vec<Mark>,
    last: Option<(f64, f64)>,
}

impl Scribble {
    pub(super) fn begin(&mut self, at: (f64, f64)) {
        self.last = Some(at);
    }

    /// Adds a mark to `at`, if a stroke is under way.
    pub(super) fn extend(&mut self, at: (f64, f64), pressure: f32) {
        if let Some(from) = self.last {
            self.marks.push(Mark {
                from,
                to: at,
                width: width_for(pressure),
            });
            self.last = Some(at);
        }
    }

    pub(super) fn end(&mut self) {
        self.last = None;
    }

    pub(super) fn clear(&mut self) {
        self.marks.clear();
        self.last = None;
    }

    pub(super) fn marks(&self) -> &[Mark] {
        &self.marks
    }
}

/// The line width for a (shaped) pressure.
pub(super) fn width_for(pressure: f32) -> f64 {
    f64::from(pressure.clamp(0.0, 1.0)) * MAX_WIDTH
}

/// The pad, shaping pressure with whatever `curve` holds at the time; and a way to clear it.
pub(super) fn scribble_pad(curve: Rc<Cell<PressureCurve>>) -> (gtk::DrawingArea, impl Fn()) {
    let area = gtk::DrawingArea::builder()
        .content_width(PAD_SIZE)
        .content_height(PAD_SIZE)
        .css_classes(["card"])
        .build();
    let scribble = Rc::new(RefCell::new(Scribble::default()));
    area.set_draw_func(glib::clone!(
        #[strong]
        scribble,
        move |area, context, _, _| draw(context, &area.color(), scribble.borrow().marks())
    ));
    attach_stylus(&area, &scribble, curve);
    let clear = glib::clone!(
        #[weak]
        area,
        #[strong]
        scribble,
        move || {
            scribble.borrow_mut().clear();
            area.queue_draw();
        }
    );
    (area, clear)
}

fn attach_stylus(
    area: &gtk::DrawingArea,
    scribble: &Rc<RefCell<Scribble>>,
    curve: Rc<Cell<PressureCurve>>,
) {
    let stylus = gtk::GestureStylus::new();
    stylus.set_stylus_only(false);
    stylus.connect_down(glib::clone!(
        #[strong]
        scribble,
        move |stylus, x, y| {
            if stylus
                .current_event()
                .is_some_and(|e| is_primary_contact(&e))
            {
                scribble.borrow_mut().begin((x, y));
            }
        }
    ));
    stylus.connect_motion(glib::clone!(
        #[strong]
        scribble,
        #[weak]
        area,
        move |stylus, x, y| {
            let Some(event) = stylus.current_event() else {
                return;
            };
            let samples = motion_samples(&event, x, y, |x, y| (x, y), &curve.get());
            let mut scribble = scribble.borrow_mut();
            for sample in samples {
                scribble.extend((sample.x, sample.y), sample.pressure);
            }
            area.queue_draw();
        }
    ));
    stylus.connect_up(glib::clone!(
        #[strong]
        scribble,
        move |_, _, _| scribble.borrow_mut().end()
    ));
    area.add_controller(stylus);
}

fn draw(context: &gtk::cairo::Context, ink: &gtk::gdk::RGBA, marks: &[Mark]) {
    context.set_source_rgba(
        f64::from(ink.red()),
        f64::from(ink.green()),
        f64::from(ink.blue()),
        MARK_ALPHA,
    );
    context.set_line_cap(gtk::cairo::LineCap::Round);
    for mark in marks {
        context.set_line_width(mark.width);
        context.move_to(mark.from.0, mark.from.1);
        context.line_to(mark.to.0, mark.to.1);
        // A failed stroke only leaves a mark undrawn.
        let _ = context.stroke();
    }
}

#[cfg(test)]
mod tests;
