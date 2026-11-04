use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::Surface;
use gtk::prelude::*;
use gtk::{gdk, glib};

use self::navigation::{pan_gesture, pinch_gesture, scroll_controller};
use self::stylus::{eyedropper_click, hover_controller, stylus_gesture};
use crate::canvas_widget::CadmiumCanvas;
use crate::state::SharedState;

const PAN_BUTTON: u32 = gdk::BUTTON_MIDDLE;

mod navigation;
mod samples;
mod stylus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanGesture {
    Primary,
    Middle,
}

impl PanGesture {
    fn button(self) -> u32 {
        match self {
            PanGesture::Primary => gdk::BUTTON_PRIMARY,
            PanGesture::Middle => PAN_BUTTON,
        }
    }

    fn index(self) -> usize {
        match self {
            PanGesture::Primary => 0,
            PanGesture::Middle => 1,
        }
    }
}

#[derive(Default)]
struct PanFlags {
    panning: [Cell<bool>; 2],
}

impl PanFlags {
    fn set(&self, gesture: PanGesture, panning: bool) {
        self.panning[gesture.index()].set(panning);
    }

    fn is_panning(&self, gesture: PanGesture) -> bool {
        self.panning[gesture.index()].get()
    }

    fn any(&self) -> bool {
        self.panning.iter().any(Cell::get)
    }
}

pub trait PaintSurfaceWidget:
    IsA<gtk::Widget> + glib::clone::Downgrade<Weak = glib::WeakRef<Self>>
{
    fn state(&self) -> SharedState;
    fn surface(&self) -> Surface;
    fn surface_point(&self, widget_x: f64, widget_y: f64) -> (f64, f64);
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
}

#[derive(Default)]
struct Contact {
    pans: PanFlags,
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
    canvas.add_controller(pan_gesture(canvas, &contact, PanGesture::Primary));
    canvas.add_controller(pan_gesture(canvas, &contact, PanGesture::Middle));
    canvas.add_controller(scroll_controller(canvas));
    canvas.add_controller(pinch_gesture(canvas));
}

fn event_belongs_to_stroke<D: PartialEq>(stroking: Option<&D>, event: Option<&D>) -> bool {
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
