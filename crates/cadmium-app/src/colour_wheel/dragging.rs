//! The pointer on the wheel: picks, rim dots dragged round or off an editable wheel, and colours
//! dropped onto it.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use cadmium_engine::palette::ColourRef;
use gtk::{gdk, glib};

use super::geometry::{is_outside, rim_hit, turn_and_radius_at, turn_at};
use super::{PickedPoint, Wheel, WheelPick, side_px};

const PAINT_PREFIX: &str = "paint";
const MINE_PREFIX: &str = "mine";

/// A colour as the text a drag carries: `paint:3`, `mine:1`.
pub(crate) fn colour_ref_text(colour: ColourRef) -> String {
    match colour {
        ColourRef::Paint(index) => format!("{PAINT_PREFIX}:{index}"),
        ColourRef::Mine(index) => format!("{MINE_PREFIX}:{index}"),
    }
}

/// The colour `colour_ref_text` wrote, or `None` for any other text.
fn parse_colour_ref(text: &str) -> Option<ColourRef> {
    let (kind, index) = text.split_once(':')?;
    let index = index.parse().ok()?;
    match kind {
        PAINT_PREFIX => Some(ColourRef::Paint(index)),
        MINE_PREFIX => Some(ColourRef::Mine(index)),
        _ => None,
    }
}

/// One pointer gesture on the wheel: the modifiers held at its press and the rim dot it holds.
struct Gesture {
    latched: Cell<gdk::ModifierType>,
    grabbed: Cell<Option<usize>>,
}

pub(super) fn attach(area: &gtk::DrawingArea, wheel: &Rc<Wheel>) {
    attach_pointer(area, wheel);
    attach_drop(area, wheel);
}

fn attach_pointer(area: &gtk::DrawingArea, wheel: &Rc<Wheel>) {
    let drag = gtk::GestureDrag::new();
    let gesture = Rc::new(Gesture {
        latched: Cell::new(gdk::ModifierType::empty()),
        grabbed: Cell::new(None),
    });
    drag.connect_drag_begin(glib::clone!(
        #[strong]
        wheel,
        #[strong]
        gesture,
        move |drag, x, y| begin(&wheel, &gesture, drag, (x, y))
    ));
    drag.connect_drag_update(glib::clone!(
        #[strong]
        wheel,
        #[strong]
        gesture,
        move |drag, dx, dy| moved(&wheel, &gesture, drag, (dx, dy))
    ));
    drag.connect_cancel(glib::clone!(
        #[strong]
        gesture,
        move |_, _| gesture.grabbed.set(None)
    ));
    drag.connect_drag_end(glib::clone!(
        #[strong]
        wheel,
        move |drag, dx, dy| ended(&wheel, &gesture, drag, (dx, dy))
    ));
    area.add_controller(drag);
}

fn begin(wheel: &Wheel, gesture: &Gesture, drag: &gtk::GestureDrag, (x, y): (f64, f64)) {
    gesture.latched.set(drag.current_event_state());
    let side_px = side_of(drag);
    let rim_len = wheel.shown.borrow().rim.len();
    let grabbed = (wheel.editable.get())
        .then(|| rim_hit(x, y, rim_len, side_px))
        .flatten();
    gesture.grabbed.set(grabbed);
    if grabbed.is_none() {
        pick(wheel, gesture, WheelPick::Press, (x, y), side_px);
    }
}

fn moved(wheel: &Wheel, gesture: &Gesture, drag: &gtk::GestureDrag, offset: (f64, f64)) {
    let Some(point) = point_of(drag, offset) else {
        return;
    };
    let side_px = side_of(drag);
    let Some(position) = gesture.grabbed.get() else {
        pick(wheel, gesture, WheelPick::Drag, point, side_px);
        return;
    };
    if !is_shown(wheel, position) {
        return;
    }
    let listener = wheel.listeners.dragged.borrow().clone();
    if let Some(listener) = listener {
        let turn = turn_at(point.0, point.1, side_px);
        gesture.grabbed.set(Some(listener(position, turn)));
    }
}

fn ended(wheel: &Wheel, gesture: &Gesture, drag: &gtk::GestureDrag, offset: (f64, f64)) {
    let Some(position) = gesture
        .grabbed
        .take()
        .filter(|position| is_shown(wheel, *position))
    else {
        return;
    };
    let Some((x, y)) = point_of(drag, offset) else {
        return;
    };
    let listener = wheel.listeners.removed.borrow().clone();
    if let Some(listener) = listener.filter(|_| is_outside(x, y, side_of(drag))) {
        listener(position);
    }
}

fn is_shown(wheel: &Wheel, position: usize) -> bool {
    position < wheel.shown.borrow().rim.len()
}

fn pick(wheel: &Wheel, gesture: &Gesture, pick: WheelPick, (x, y): (f64, f64), side_px: f64) {
    let Some((turn, radius)) = turn_and_radius_at(x, y, side_px) else {
        return;
    };
    let listener = wheel.listeners.picked.borrow().clone();
    if let Some(listener) = listener {
        listener(PickedPoint {
            turn,
            radius,
            pick,
            modifiers: gesture.latched.get(),
        });
    }
}

fn point_of(drag: &gtk::GestureDrag, (dx, dy): (f64, f64)) -> Option<(f64, f64)> {
    drag.start_point().map(|(x, y)| (x + dx, y + dy))
}

fn side_of(controller: &impl IsA<gtk::EventController>) -> f64 {
    controller
        .widget()
        .map_or(0.0, |area| side_px(area.width(), area.height()))
}

fn attach_drop(area: &gtk::DrawingArea, wheel: &Rc<Wheel>) {
    let target = gtk::DropTarget::new(
        glib::Type::STRING,
        gdk::DragAction::COPY | gdk::DragAction::MOVE,
    );
    target.set_preload(true);
    target.connect_value_notify(reject_foreign_text);
    target.connect_accept(glib::clone!(
        #[strong]
        wheel,
        move |_, drop| accepts(&wheel, drop)
    ));
    target.connect_drop(glib::clone!(
        #[strong]
        wheel,
        move |target, value, x, y| dropped(&wheel, target, value, (x, y))
    ));
    area.add_controller(target);
}

/// Only a drag started in this app can carry a colour; text from elsewhere is not one.
fn accepts(wheel: &Wheel, drop: &gdk::Drop) -> bool {
    wheel.editable.get()
        && drop.drag().is_some()
        && drop.formats().contains_type(glib::Type::STRING)
}

/// Once the dragged text has loaded, a text that names no colour is turned away, so the wheel
/// does not offer itself as a place to drop it.
fn reject_foreign_text(target: &gtk::DropTarget) {
    let Some(value) = target.value() else {
        return;
    };
    let text = value.get::<String>().ok();
    if text.as_deref().and_then(parse_colour_ref).is_none() {
        target.reject();
    }
}

fn dropped(
    wheel: &Wheel,
    target: &gtk::DropTarget,
    value: &glib::Value,
    (x, y): (f64, f64),
) -> bool {
    let text = value.get::<String>().ok();
    let colour = text.as_deref().and_then(parse_colour_ref);
    let listener = wheel.listeners.dropped.borrow().clone();
    let (Some(colour), Some(listener)) = (colour.filter(|_| wheel.editable.get()), listener) else {
        return false;
    };
    listener(colour, turn_at(x, y, side_of(target)));
    true
}

#[cfg(test)]
mod tests;
