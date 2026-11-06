use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::Surface;
use gtk::prelude::*;
use gtk::{gdk, glib};

use self::nav_drag::{NavButton, NavDrag};
use self::navigation::{nav_gesture, pinch_gesture, scroll_controller};
use self::stylus::{eyedropper_click, hover_controller, stylus_gesture};
use crate::canvas_widget::CadmiumCanvas;
use crate::state::SharedState;

pub(crate) mod nav_drag;
mod navigation;
pub(crate) mod samples;
mod stylus;

/// The navigation drag each button has in progress.
#[derive(Default)]
struct NavDrags {
    drags: [Cell<Option<NavDrag>>; 3],
}

impl NavDrags {
    fn set(&self, button: NavButton, drag: Option<NavDrag>) {
        self.drags[button.index()].set(drag);
    }

    fn get(&self, button: NavButton) -> Option<NavDrag> {
        self.drags[button.index()].get()
    }

    fn any(&self) -> bool {
        self.drags.iter().any(|drag| drag.get().is_some())
    }
}

pub trait PaintSurfaceWidget:
    IsA<gtk::Widget> + glib::clone::Downgrade<Weak = glib::WeakRef<Self>>
{
    fn state(&self) -> SharedState;
    fn surface(&self) -> Surface;
    fn surface_point(&self, widget_x: f64, widget_y: f64) -> (f64, f64);
    /// Clockwise turn of the surface's picture on screen, in radians.
    fn view_rotation(&self) -> f32 {
        0.0
    }
    fn set_hover(&self, position: Option<(f64, f64)>);
    fn pick_colour_at(&self, widget_x: f64, widget_y: f64);
    fn observe_hover_pen(&self, sample: &StrokeSample);
    fn accepts_point(&self, _widget_x: f64, _widget_y: f64) -> bool {
        true
    }
    fn claims_strokes(&self) -> bool {
        false
    }
    fn contact_released(&self) {}
    /// Whether held keys (Space here, or `modifiers` from the contact's event) have the
    /// primary contact navigate instead of paint.
    fn holds_navigation(&self, _modifiers: gdk::ModifierType) -> bool {
        false
    }
}

#[derive(Default)]
struct Contact {
    drags: NavDrags,
    stroking_device: RefCell<Option<gdk::Device>>,
}

impl Contact {
    fn is_stroking_device(&self, device: Option<&gdk::Device>) -> bool {
        event_belongs_to_stroke(self.stroking_device.borrow().as_ref(), device)
    }
}

fn attach_stylus_sharing<W: PaintSurfaceWidget>(widget: &W, contact: &Rc<Contact>) {
    widget.add_controller(stylus_gesture(widget, contact));
    widget.add_controller(eyedropper_click(widget));
    widget.add_controller(hover_controller(widget, contact));
}

pub fn attach_stylus<W: PaintSurfaceWidget>(widget: &W) {
    attach_stylus_sharing(widget, &Rc::new(Contact::default()));
}

pub fn attach(canvas: &CadmiumCanvas) {
    let contact = Rc::new(Contact::default());
    attach_stylus_sharing(canvas, &contact);
    for button in NavButton::ALL {
        canvas.add_controller(nav_gesture(canvas, &contact, button));
    }
    canvas.add_controller(scroll_controller(canvas));
    canvas.add_controller(pinch_gesture(canvas));
}

/// Whether an event from `event`'s device belongs to the stroke `stroking` paints; with
/// either device unknown, it does.
pub(crate) fn event_belongs_to_stroke<D: PartialEq>(
    stroking: Option<&D>,
    event: Option<&D>,
) -> bool {
    match (stroking, event) {
        (Some(stroking), Some(event)) => stroking == event,
        _ => true,
    }
}

fn side_button_may_act(stroking: bool) -> bool {
    !stroking
}

fn stroke_may_begin(panning: bool) -> bool {
    !panning
}

fn release<W: PaintSurfaceWidget>(widget: &W, contact: &Contact) {
    contact.stroking_device.replace(None);
    let state = widget.state();
    if !state.borrow().contact_on(widget.surface()) {
        return;
    }
    state.borrow_mut().release();
    widget.contact_released();
}

#[cfg(test)]
mod tests;
