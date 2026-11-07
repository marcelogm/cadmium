//! A small pad to try the curve on: each move leaves a line as wide as the shaped pressure.
//! Plain cairo lines, not the paint engine.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::input::samples::{is_primary_contact, motion_samples};
use crate::pressure_curve::PressureCurve;

/// The width of a line at full pressure, in widget px.
const MAX_WIDTH: f64 = 24.0;
const PAD_SIZE: i32 = 200;
/// The most marks kept; the oldest go first.
const MAX_MARKS: usize = 4096;

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
    marks: VecDeque<Mark>,
    last: Option<(f64, f64)>,
}

impl Scribble {
    pub(super) fn begin(&mut self, at: (f64, f64)) {
        self.last = Some(at);
    }

    /// Adds a mark to `at`, if a stroke is under way; whether it did.
    pub(super) fn extend(&mut self, at: (f64, f64), pressure: f32) -> bool {
        let Some(from) = self.last else {
            return false;
        };
        if self.marks.len() >= MAX_MARKS {
            self.marks.pop_front();
        }
        self.marks.push_back(Mark {
            from,
            to: at,
            width: width_for(pressure),
        });
        self.last = Some(at);
        true
    }

    pub(super) fn end(&mut self) {
        self.last = None;
    }

    pub(super) fn clear(&mut self) {
        self.marks.clear();
        self.last = None;
    }

    pub(super) fn marks(&self) -> &VecDeque<Mark> {
        &self.marks
    }
}

/// The line width for a (shaped) pressure.
fn width_for(pressure: f32) -> f64 {
    f64::from(pressure.clamp(0.0, 1.0)) * MAX_WIDTH
}

/// The pad, shaping pressure with whatever `curve` holds at the time; and a way to clear it.
pub(super) fn scribble_pad(curve: Rc<Cell<PressureCurve>>) -> (gtk::DrawingArea, impl Fn()) {
    let area = gtk::DrawingArea::builder()
        .content_width(PAD_SIZE)
        .content_height(PAD_SIZE)
        .css_classes(["card"])
        .build();
    area.update_property(&[gtk::accessible::Property::Label("Pressure test pad")]);
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
            if is_primary(stylus) {
                // Keeps a touch scribble from scrolling the page.
                stylus.set_state(gtk::EventSequenceState::Claimed);
                scribble.borrow_mut().begin((x, y));
            }
        }
    ));
    stylus.connect_motion(glib::clone!(
        #[strong]
        scribble,
        #[weak]
        area,
        move |stylus, x, y| motion(stylus, (x, y), &scribble, &area, &curve.get())
    ));
    stylus.connect_up(glib::clone!(
        #[strong]
        scribble,
        move |stylus, _, _| {
            if is_primary(stylus) {
                scribble.borrow_mut().end();
            }
        }
    ));
    stylus.connect_cancel(glib::clone!(
        #[strong]
        scribble,
        move |_, _| scribble.borrow_mut().end()
    ));
    area.add_controller(stylus);
}

fn is_primary(stylus: &gtk::GestureStylus) -> bool {
    stylus
        .current_event()
        .is_some_and(|event| is_primary_contact(&event))
}

fn motion(
    stylus: &gtk::GestureStylus,
    (x, y): (f64, f64),
    scribble: &RefCell<Scribble>,
    area: &gtk::DrawingArea,
    curve: &PressureCurve,
) {
    let Some(event) = stylus.current_event() else {
        return;
    };
    let mut scribble = scribble.borrow_mut();
    let mut added = false;
    for sample in motion_samples(&event, x, y, |x, y| (x, y), curve) {
        added |= scribble.extend((sample.x, sample.y), sample.pressure);
    }
    if added {
        area.queue_draw();
    }
}

fn draw(context: &gtk::cairo::Context, ink: &gtk::gdk::RGBA, marks: &VecDeque<Mark>) {
    context.set_source_rgb(
        f64::from(ink.red()),
        f64::from(ink.green()),
        f64::from(ink.blue()),
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
