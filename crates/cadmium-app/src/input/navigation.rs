use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use super::nav_drag::{NavButton, NavDrag, NavPress, held_mode, nav_mode};
use super::samples::{comes_from_a_pen, pen_buttons_of};
use super::scroll::{ScrollAction, ScrollUnit, pinch_factor, scroll_action};
use super::{Contact, NavigableSurface};
use crate::tools::Press;
use crate::view::ViewStep;

/// The drag that navigates the view from `button`, when the press calls for it.
pub(super) fn nav_gesture<W: NavigableSurface>(
    surface: &W,
    contact: &Rc<Contact>,
    button: NavButton,
) -> gtk::GestureDrag {
    let drag = gtk::GestureDrag::new();
    drag.set_button(button.button());
    drag.connect_drag_begin(glib::clone!(
        #[weak]
        surface,
        #[strong]
        contact,
        move |drag, x, y| begin_nav(&surface, &contact, drag, button, (x, y))
    ));
    drag.connect_drag_update(glib::clone!(
        #[weak]
        surface,
        #[strong]
        contact,
        move |_, offset_x, offset_y| continue_nav(&surface, &contact, button, (offset_x, offset_y))
    ));
    drag.connect_drag_end(glib::clone!(
        #[strong]
        contact,
        move |_, _, _| contact.drags.set(button, None)
    ));
    drag
}

/// Starts `button`'s navigation drag, or denies the drag when the press doesn't navigate.
fn begin_nav<W: NavigableSurface>(
    surface: &W,
    contact: &Contact,
    drag: &gtk::GestureDrag,
    button: NavButton,
    start: (f64, f64),
) {
    let state = surface.state();
    let (press, settings) = {
        let state = state.borrow();
        let press = NavPress {
            button,
            from_pen: drag
                .current_event()
                .is_some_and(|event| comes_from_a_pen(&event)),
            // The pointer event's own modifier state is current; key events' is one step behind.
            held: held_mode(surface.space_held(), drag.current_event_state()),
            tool_pans: state.press_for(surface.surface()) == Press::Pan,
            stroking: state.is_stroking(),
            pen_buttons: pen_buttons_of(surface),
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
fn continue_nav<W: NavigableSurface>(
    surface: &W,
    contact: &Contact,
    button: NavButton,
    offset: (f64, f64),
) {
    let Some(mut drag) = contact.drags.get(button) else {
        return;
    };
    let step = drag.step(offset, surface.widget_centre());
    contact.drags.set(button, Some(drag));
    if let Some(step) = step {
        surface.apply_view_step(step);
    }
}

pub(super) fn scroll_controller<W: NavigableSurface>(surface: &W) -> gtk::EventControllerScroll {
    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    scroll.connect_scroll(glib::clone!(
        #[weak]
        surface,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |controller, dx, dy| scroll_view(&surface, controller, dx, dy)
    ));
    scroll
}

fn scroll_view<W: NavigableSurface>(
    surface: &W,
    controller: &gtk::EventControllerScroll,
    dx: f64,
    dy: f64,
) -> glib::Propagation {
    let unit = match controller.unit() {
        gdk::ScrollUnit::Wheel => ScrollUnit::Wheel,
        _ => ScrollUnit::Surface,
    };
    let shift = controller
        .current_event_state()
        .contains(gdk::ModifierType::SHIFT_MASK);
    match scroll_action(unit, shift, dx, dy) {
        ScrollAction::Pan { dx, dy } => {
            surface.pan_by(dx, dy);
            glib::Propagation::Stop
        }
        ScrollAction::Zoom { factor } => {
            let anchor = surface.pointer().unwrap_or(surface.widget_centre());
            surface.apply_view_step(ViewStep::Zoom { anchor, factor });
            glib::Propagation::Stop
        }
        ScrollAction::Pass => glib::Propagation::Proceed,
    }
}

pub(super) fn pinch_gesture<W: NavigableSurface>(surface: &W) -> gtk::GestureZoom {
    let pinch = gtk::GestureZoom::new();
    let last_scale = Rc::new(Cell::new(1.0f64));
    pinch.connect_begin(glib::clone!(
        #[strong]
        last_scale,
        move |_, _| last_scale.set(1.0)
    ));
    pinch.connect_scale_changed(glib::clone!(
        #[weak]
        surface,
        move |pinch, scale| pinch_zoom(&surface, &last_scale, pinch, scale)
    ));
    pinch
}

fn pinch_zoom<W: NavigableSurface>(
    surface: &W,
    last_scale: &Cell<f64>,
    pinch: &gtk::GestureZoom,
    scale: f64,
) {
    let previous = last_scale.replace(scale);
    if let Some(factor) = pinch_factor(previous, scale)
        && let Some(anchor) = pinch.bounding_box_center()
    {
        surface.apply_view_step(ViewStep::Zoom { anchor, factor });
    }
}
