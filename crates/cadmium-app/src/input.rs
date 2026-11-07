use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cadmium_engine::stroke::StrokeSample;
use cadmium_engine::surface::Surface;
use gtk::prelude::*;
use gtk::{gdk, glib};

use self::nav_drag::{NavButton, NavDrag};
use self::navigation::{nav_gesture, pinch_gesture, scroll_controller};
use self::stylus::{eyedropper_click, hover_controller, stylus_gesture};
use crate::state::SharedState;
use crate::tools::Press;
use crate::view::ViewStep;

pub(crate) mod nav_drag;
mod navigation;
pub(crate) mod samples;
mod scroll;
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

/// A paint surface the painter navigates: scrolls pan or zoom, pinches zoom, and drags pan,
/// zoom or turn its view, the same on the canvas and the tray.
pub trait NavigableSurface: PaintSurfaceWidget {
    /// Whether the window holds Space down for this surface.
    fn space_held(&self) -> bool;
    /// The widget's centre, in widget px.
    fn widget_centre(&self) -> (f64, f64);
    /// Where the pointer is over the widget, to zoom about.
    fn pointer(&self) -> Option<(f64, f64)>;
    /// Applies one step of a navigation gesture to the view.
    fn apply_view_step(&self, step: ViewStep);

    fn pan_by(&self, dx: f64, dy: f64) {
        self.apply_view_step(ViewStep::Pan { dx, dy });
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

/// Lets the pen paint on `surface` and the painter navigate its view.
pub fn attach<W: NavigableSurface>(surface: &W) {
    let contact = Rc::new(Contact::default());
    attach_stylus_sharing(surface, &contact);
    for button in NavButton::ALL {
        surface.add_controller(nav_gesture(surface, &contact, button));
    }
    surface.add_controller(scroll_controller(surface));
    surface.add_controller(pinch_gesture(surface));
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

/// What a pen touching down does under a tool that presses as `press`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DownOutcome {
    Ignore,
    PickColour,
    /// Hand the touch to the painting, as a stroke or a wipe.
    Press,
}

/// The Move tool leaves the pen to navigation; the eyedropper picks with the tip, while the
/// eraser end still wipes; every other press goes to the painting.
fn down_outcome(press: Press, eraser: bool) -> DownOutcome {
    match press {
        Press::Pan => DownOutcome::Ignore,
        Press::Nothing if !eraser => DownOutcome::Ignore,
        Press::PickColour if !eraser => DownOutcome::PickColour,
        Press::Nothing
        | Press::PickColour
        | Press::Stroke(_)
        | Press::LoadFromPile(_)
        | Press::Squeeze => DownOutcome::Press,
    }
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
