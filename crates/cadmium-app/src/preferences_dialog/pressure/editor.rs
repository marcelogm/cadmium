//! The curve editor: the curve on a grid, its points as handles. A press on empty space adds a
//! point, a drag moves one, a double-click removes one. A finished change is handed on.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, gdk, glib};

use super::editing::{drag, grab, point_near, remove};
use super::plot::Plot;
use crate::pressure_curve::PressureCurve;

const EDITOR_SIZE: i32 = 200;
/// How near a press must be to a handle to take it, in widget px.
const HIT_RADIUS: f64 = 10.0;
const HANDLE_RADIUS: f64 = 5.0;
const GRID_DIVISIONS: u32 = 4;
const CURVE_STEPS: u32 = 128;
const GRID_ALPHA: f64 = 0.15;
const DIAGONAL_ALPHA: f64 = 0.3;

/// The editor's shared state: the curve as shown, the point held (with its handle's widget
/// position) and the curve before the hold.
struct Editing {
    curve: Rc<Cell<PressureCurve>>,
    held: Cell<Option<(usize, (f64, f64))>>,
    before: Cell<PressureCurve>,
    on_change: Box<dyn Fn(PressureCurve)>,
}

/// An editor over `curve`; `on_change` gets each finished change (a press, a drag, a removal).
pub(super) fn curve_editor(
    curve: Rc<Cell<PressureCurve>>,
    on_change: impl Fn(PressureCurve) + 'static,
) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(EDITOR_SIZE)
        .content_height(EDITOR_SIZE)
        .css_classes(["card"])
        .build();
    area.update_property(&[gtk::accessible::Property::Label("Pressure curve")]);
    let editing = Rc::new(Editing {
        before: Cell::new(curve.get()),
        curve,
        held: Cell::new(None),
        on_change: Box::new(on_change),
    });
    area.set_draw_func(glib::clone!(
        #[strong]
        editing,
        move |area, context, width, height| {
            let plot = Plot::in_widget(f64::from(width), f64::from(height));
            draw(context, &area.color(), plot, &editing.curve.get());
        }
    ));
    let click = attach_click(&area, &editing);
    attach_drag(&area, &editing, &click);
    area
}

fn plot_of(area: &gtk::DrawingArea) -> Plot {
    Plot::in_widget(f64::from(area.width()), f64::from(area.height()))
}

/// The first press takes hold of a point (adding one on empty space); the second of a
/// double-click removes it.
fn attach_click(area: &gtk::DrawingArea, editing: &Rc<Editing>) -> gtk::GestureClick {
    let click = gtk::GestureClick::new();
    click.connect_pressed(glib::clone!(
        #[strong]
        editing,
        #[weak]
        area,
        move |click, presses, x, y| {
            let plot = plot_of(&area);
            match presses {
                1 => {
                    // A press that took or added a point claims the touch; one with no room
                    // for a point leaves it to the page to scroll.
                    if hold(&editing, plot, (x, y)) {
                        click.set_state(gtk::EventSequenceState::Claimed);
                    }
                }
                2 => remove_at(&editing, plot, (x, y)),
                _ => {}
            }
            area.queue_draw();
        }
    ));
    area.add_controller(click.clone());
    click
}

/// Takes the point at the press, remembering where its handle is so a drag moves the handle
/// by the pointer's offset rather than snapping it to the press.
fn hold(editing: &Editing, plot: Plot, (x, y): (f64, f64)) -> bool {
    let curve = editing.curve.get();
    editing.before.set(curve);
    let radius = plot.unit_distance(HIT_RADIUS);
    if let Some((held, index)) = grab(&curve, plot.to_unit(x, y), radius) {
        editing.curve.set(held);
        let handle = plot.to_widget(held.points()[index]);
        editing.held.set(Some((index, handle)));
        return true;
    }
    false
}

fn remove_at(editing: &Editing, plot: Plot, (x, y): (f64, f64)) {
    editing.held.set(None);
    let curve = editing.curve.get();
    let Some(index) = point_near(&curve, plot.to_unit(x, y), plot.unit_distance(HIT_RADIUS)) else {
        return;
    };
    let removed = remove(&curve, index);
    if removed != curve {
        editing.curve.set(removed);
        (editing.on_change)(removed);
    }
}

/// Moves the held point with the pointer; the release hands on the change.
fn attach_drag(area: &gtk::DrawingArea, editing: &Rc<Editing>, click: &gtk::GestureClick) {
    let gesture = gtk::GestureDrag::new();
    // Grouped, so claiming the click's sequence doesn't deny the drag.
    gesture.group_with(click);
    gesture.connect_drag_update(glib::clone!(
        #[strong]
        editing,
        #[weak]
        area,
        move |_, dx, dy| {
            let Some((index, (x, y))) = editing.held.get() else {
                return;
            };
            let to = plot_of(&area).to_unit(x + dx, y + dy);
            editing.curve.set(drag(&editing.curve.get(), index, to));
            area.queue_draw();
        }
    ));
    gesture.connect_drag_end(glib::clone!(
        #[strong]
        editing,
        move |_, _, _| {
            if editing.held.take().is_some() && editing.curve.get() != editing.before.get() {
                (editing.on_change)(editing.curve.get());
            }
        }
    ));
    area.add_controller(gesture);
}

fn draw(context: &cairo::Context, ink: &gdk::RGBA, plot: Plot, curve: &PressureCurve) {
    let (left, top, side) = plot.bounds();
    let set_ink = |alpha: f64| {
        context.set_source_rgba(
            f64::from(ink.red()),
            f64::from(ink.green()),
            f64::from(ink.blue()),
            alpha,
        );
    };
    context.set_line_width(1.0);
    set_ink(GRID_ALPHA);
    for step in 0..=GRID_DIVISIONS {
        let at = f64::from(step) / f64::from(GRID_DIVISIONS) * side;
        context.move_to(left + at, top);
        context.line_to(left + at, top + side);
        context.move_to(left, top + at);
        context.line_to(left + side, top + at);
    }
    // Failed strokes only leave part of the picture undrawn.
    let _ = context.stroke();
    set_ink(DIAGONAL_ALPHA);
    context.move_to(left, top + side);
    context.line_to(left + side, top);
    let _ = context.stroke();
    set_ink(1.0);
    draw_curve(context, plot, curve);
    draw_handles(context, plot, curve);
}

fn draw_curve(context: &cairo::Context, plot: Plot, curve: &PressureCurve) {
    context.set_line_width(2.0);
    for step in 0..=CURVE_STEPS {
        let x = step as f32 / CURVE_STEPS as f32;
        let (wx, wy) = plot.to_widget((x, curve.apply(x)));
        context.line_to(wx, wy);
    }
    let _ = context.stroke();
}

fn draw_handles(context: &cairo::Context, plot: Plot, curve: &PressureCurve) {
    for &point in curve.points() {
        let (x, y) = plot.to_widget(point);
        context.new_sub_path();
        context.arc(x, y, HANDLE_RADIUS, 0.0, std::f64::consts::TAU);
    }
    let _ = context.fill();
}
