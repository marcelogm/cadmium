use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use super::nav_drag::{NavButton, NavDrag, NavPress, nav_mode, space_mode};
use super::samples::{comes_from_a_pen, pen_buttons_of};
use super::{Contact, PaintSurfaceWidget};
use crate::canvas_widget::CadmiumCanvas;
use crate::tools::Press;

const ZOOM_PER_SCROLL_STEP: f32 = 1.1;
const SURFACE_SCROLL_PER_STEP: f64 = 15.0;
const WHEEL_PAN_STEP: f64 = 48.0;

/// The drag that navigates the view from `button`, when the press calls for it.
pub(super) fn nav_gesture(
    canvas: &CadmiumCanvas,
    contact: &Rc<Contact>,
    button: NavButton,
) -> gtk::GestureDrag {
    let drag = gtk::GestureDrag::new();
    drag.set_button(button.button());
    drag.connect_drag_begin(glib::clone!(
        #[weak]
        canvas,
        #[strong]
        contact,
        move |drag, x, y| begin_nav(&canvas, &contact, drag, button, (x, y))
    ));
    drag.connect_drag_update(glib::clone!(
        #[weak]
        canvas,
        #[strong]
        contact,
        move |_, offset_x, offset_y| continue_nav(&canvas, &contact, button, (offset_x, offset_y))
    ));
    drag.connect_drag_end(glib::clone!(
        #[strong]
        contact,
        move |_, _, _| contact.drags.set(button, None)
    ));
    drag
}

/// Starts `button`'s navigation drag, or denies the drag when the press doesn't navigate.
fn begin_nav(
    canvas: &CadmiumCanvas,
    contact: &Contact,
    drag: &gtk::GestureDrag,
    button: NavButton,
    start: (f64, f64),
) {
    let state = canvas.state();
    let (press, settings) = {
        let state = state.borrow();
        let press = NavPress {
            button,
            from_pen: drag
                .current_event()
                .is_some_and(|event| comes_from_a_pen(&event)),
            // The pointer event's own modifier state is current; key events' is one step behind.
            space: canvas
                .space_mode()
                .map(|_| space_mode(drag.current_event_state())),
            tool_pans: state.ui_tool.profile().press_on(canvas.surface()) == Press::Pan,
            stroking: state.is_stroking(),
            pen_buttons: pen_buttons_of(canvas),
        };
        (press, state.settings.get())
    };
    let started = nav_mode(press, settings).map(|mode| NavDrag::new(mode, start));
    contact.drags.set(button, started);
    if started.is_none() {
        drag.set_state(gtk::EventSequenceState::Denied);
    }
}

/// Steps the view along `button`'s drag, now `offset` from where it started.
fn continue_nav(canvas: &CadmiumCanvas, contact: &Contact, button: NavButton, offset: (f64, f64)) {
    let Some(mut drag) = contact.drags.get(button) else {
        return;
    };
    let step = drag.step(offset, canvas.widget_centre());
    contact.drags.set(button, Some(drag));
    if let Some(step) = step {
        canvas.apply_view_step(step);
    }
}

fn scroll_steps(controller: &gtk::EventControllerScroll, delta: f64) -> f64 {
    match controller.unit() {
        gdk::ScrollUnit::Wheel => delta,
        _ => delta / SURFACE_SCROLL_PER_STEP,
    }
}

pub(super) fn scroll_controller(canvas: &CadmiumCanvas) -> gtk::EventControllerScroll {
    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    scroll.connect_scroll(glib::clone!(
        #[weak]
        canvas,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |controller, dx, dy| scroll_view(&canvas, controller, dx, dy)
    ));
    scroll
}

fn scroll_view(
    canvas: &CadmiumCanvas,
    controller: &gtk::EventControllerScroll,
    dx: f64,
    dy: f64,
) -> glib::Propagation {
    let shift = controller
        .current_event_state()
        .contains(gdk::ModifierType::SHIFT_MASK);
    if shift {
        pan_by_scroll(canvas, controller, dx, dy);
        return glib::Propagation::Stop;
    }
    if dy == 0.0 {
        return glib::Propagation::Proceed;
    }
    let (x, y) = canvas.hover().unwrap_or((
        f64::from(canvas.width()) / 2.0,
        f64::from(canvas.height()) / 2.0,
    ));
    let factor = ZOOM_PER_SCROLL_STEP.powf(-scroll_steps(controller, dy) as f32);
    canvas.set_zoom_around(x, y, canvas.zoom() * factor);
    glib::Propagation::Stop
}

fn pan_by_scroll(
    canvas: &CadmiumCanvas,
    controller: &gtk::EventControllerScroll,
    dx: f64,
    dy: f64,
) {
    let step = match controller.unit() {
        gdk::ScrollUnit::Wheel => WHEEL_PAN_STEP,
        _ => 1.0,
    };
    let horizontal = if dx == 0.0 { dy } else { dx };
    let vertical = if dx == 0.0 { 0.0 } else { dy };
    canvas.pan_by(-horizontal * step, -vertical * step);
}

pub(super) fn pinch_gesture(canvas: &CadmiumCanvas) -> gtk::GestureZoom {
    let pinch = gtk::GestureZoom::new();
    let last_scale = Rc::new(Cell::new(1.0f64));
    pinch.connect_begin(glib::clone!(
        #[strong]
        last_scale,
        move |_, _| last_scale.set(1.0)
    ));
    pinch.connect_scale_changed(glib::clone!(
        #[weak]
        canvas,
        move |pinch, scale| pinch_zoom(&canvas, &last_scale, pinch, scale)
    ));
    pinch
}

fn pinch_zoom(
    canvas: &CadmiumCanvas,
    last_scale: &Cell<f64>,
    pinch: &gtk::GestureZoom,
    scale: f64,
) {
    let previous = last_scale.replace(scale);
    if previous <= 0.0 || scale <= 0.0 {
        return;
    }
    if let Some((x, y)) = pinch.bounding_box_center() {
        canvas.set_zoom_around(x, y, canvas.zoom() * (scale / previous) as f32);
    }
}
