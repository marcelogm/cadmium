use std::rc::Rc;

use cadmium_engine::stroke::StrokeSample;
use gtk::prelude::*;
use gtk::{gdk, glib};

use super::samples::{
    comes_from_a_pen, event_sample, is_eraser, is_primary_contact, motion_samples, tilt_onto_canvas,
};
use super::{Contact, PaintSurfaceWidget, release, side_button_may_act, stroke_may_begin};
use crate::tools::Press;

const EYEDROPPER_BUTTON: u32 = gdk::BUTTON_SECONDARY;

pub(super) fn stylus_gesture<W: PaintSurfaceWidget>(
    widget: &W,
    contact: &Rc<Contact>,
) -> gtk::GestureStylus {
    let stylus = gtk::GestureStylus::new();
    stylus.set_stylus_only(false);
    stylus.connect_down(glib::clone!(
        #[weak]
        widget,
        #[strong]
        contact,
        move |stylus, x, y| stylus_down(&widget, &contact, stylus, x, y)
    ));
    stylus.connect_motion(glib::clone!(
        #[weak]
        widget,
        #[strong]
        contact,
        move |stylus, x, y| stylus_motion(&widget, &contact, stylus, x, y)
    ));
    stylus.connect_up(glib::clone!(
        #[weak]
        widget,
        #[strong]
        contact,
        move |stylus, _, _| stylus_up(&widget, &contact, stylus)
    ));
    connect_proximity_and_cancel(&stylus, widget, contact);
    stylus
}

fn connect_proximity_and_cancel<W: PaintSurfaceWidget>(
    stylus: &gtk::GestureStylus,
    widget: &W,
    contact: &Rc<Contact>,
) {
    stylus.connect_proximity(glib::clone!(
        #[weak]
        widget,
        #[strong]
        contact,
        move |stylus, x, y| stylus_proximity(&widget, &contact, stylus, x, y)
    ));
    stylus.connect_cancel(glib::clone!(
        #[weak]
        widget,
        #[strong]
        contact,
        move |_, _| release(&widget, &contact)
    ));
}

/// The pen's state at widget point (`x`, `y`), mapped onto the widget's surface.
fn widget_sample<W: PaintSurfaceWidget>(
    widget: &W,
    event: &gdk::Event,
    x: f64,
    y: f64,
) -> StrokeSample {
    let sample = event_sample(event, x, y, |x, y| widget.surface_point(x, y));
    tilt_onto_canvas(sample, widget.view_rotation())
}

fn stylus_down<W: PaintSurfaceWidget>(
    widget: &W,
    contact: &Contact,
    stylus: &gtk::GestureStylus,
    x: f64,
    y: f64,
) {
    let Some(event) = stylus.current_event() else {
        return;
    };
    if !is_primary_contact(&event)
        || !stroke_may_begin(contact.pans.any())
        || !widget.accepts_point(x, y)
    {
        return;
    }
    let state = widget.state();
    let press = state.borrow().ui_tool.profile().press_on(widget.surface());
    let eraser = is_eraser(&event);
    if press == Press::Pan {
        return;
    }
    if !eraser && press == Press::PickColour {
        widget.pick_colour_at(x, y);
        return;
    }
    let sample = widget_sample(widget, &event, x, y);
    if !state.borrow_mut().press(widget.surface(), sample, eraser) {
        return;
    }
    if widget.claims_strokes() {
        stylus.set_state(gtk::EventSequenceState::Claimed);
    }
    contact
        .stroking_device
        .replace(stylus.current_event_device());
    widget.set_hover(Some((x, y)));
}

fn stylus_motion<W: PaintSurfaceWidget>(
    widget: &W,
    contact: &Contact,
    stylus: &gtk::GestureStylus,
    x: f64,
    y: f64,
) {
    widget.set_hover(Some((x, y)));
    let Some(event) = stylus.current_event() else {
        return;
    };
    let state = widget.state();
    if !state.borrow().contact_on(widget.surface()) {
        if comes_from_a_pen(&event) {
            widget.observe_hover_pen(&widget_sample(widget, &event, x, y));
        }
        return;
    }
    if !contact.is_stroking_device(stylus.current_event_device().as_ref()) {
        return;
    }
    let mut samples = motion_samples(&event, x, y, |x, y| widget.surface_point(x, y));
    let rotation = widget.view_rotation();
    for sample in &mut samples {
        *sample = tilt_onto_canvas(*sample, rotation);
    }
    state.borrow_mut().motion(&samples);
}

fn stylus_up<W: PaintSurfaceWidget>(widget: &W, contact: &Contact, stylus: &gtk::GestureStylus) {
    if stylus
        .current_event()
        .is_some_and(|event| is_primary_contact(&event))
    {
        release(widget, contact);
    }
}

fn stylus_proximity<W: PaintSurfaceWidget>(
    widget: &W,
    contact: &Contact,
    stylus: &gtk::GestureStylus,
    x: f64,
    y: f64,
) {
    let Some(event) = stylus.current_event() else {
        return;
    };
    if event.event_type() == gdk::EventType::ProximityOut {
        if contact.is_stroking_device(stylus.current_event_device().as_ref()) {
            release(widget, contact);
        }
        widget.set_hover(None);
        return;
    }
    widget.set_hover(Some((x, y)));
    if comes_from_a_pen(&event) {
        widget.observe_hover_pen(&widget_sample(widget, &event, x, y));
    }
}

pub(super) fn eyedropper_click<W: PaintSurfaceWidget>(widget: &W) -> gtk::GestureClick {
    let click = gtk::GestureClick::new();
    click.set_button(EYEDROPPER_BUTTON);
    click.connect_pressed(glib::clone!(
        #[weak]
        widget,
        move |_, _, x, y| pick_with_side_button(&widget, x, y)
    ));
    click
}

fn pick_with_side_button<W: PaintSurfaceWidget>(widget: &W, x: f64, y: f64) {
    if side_button_may_act(widget.state().borrow().is_stroking()) && widget.accepts_point(x, y) {
        widget.pick_colour_at(x, y);
    }
}

pub(super) fn hover_controller<W: PaintSurfaceWidget>(
    widget: &W,
    contact: &Rc<Contact>,
) -> gtk::EventControllerMotion {
    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter(glib::clone!(
        #[weak]
        widget,
        move |_, x, y| widget.set_hover(Some((x, y)))
    ));
    motion.connect_motion(glib::clone!(
        #[weak]
        widget,
        move |_, x, y| widget.set_hover(Some((x, y)))
    ));
    motion.connect_leave(glib::clone!(
        #[weak]
        widget,
        #[strong]
        contact,
        move |motion| hover_left(&widget, &contact, motion)
    ));
    motion
}

fn hover_left<W: PaintSurfaceWidget>(
    widget: &W,
    contact: &Contact,
    motion: &gtk::EventControllerMotion,
) {
    widget.set_hover(None);
    if contact.is_stroking_device(motion.current_event_device().as_ref()) {
        release(widget, contact);
    }
}
