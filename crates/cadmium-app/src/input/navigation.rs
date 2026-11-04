use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use super::{Contact, PaintSurfaceWidget, PanGesture, side_button_may_act};
use crate::canvas_widget::CadmiumCanvas;
use crate::tools::Press;

const ZOOM_PER_SCROLL_STEP: f32 = 1.1;
const SURFACE_SCROLL_PER_STEP: f64 = 15.0;
const WHEEL_PAN_STEP: f64 = 48.0;

pub(super) fn pan_gesture(
    canvas: &CadmiumCanvas,
    contact: &Rc<Contact>,
    gesture: PanGesture,
) -> gtk::GestureDrag {
    let drag = gtk::GestureDrag::new();
    drag.set_button(gesture.button());
    let last_offset = Rc::new(Cell::new((0.0, 0.0)));
    drag.connect_drag_begin(glib::clone!(
        #[weak]
        canvas,
        #[strong]
        contact,
        #[strong]
        last_offset,
        move |drag, _, _| begin_pan(&canvas, &contact, &last_offset, drag, gesture)
    ));
    drag.connect_drag_update(glib::clone!(
        #[weak]
        canvas,
        #[strong]
        contact,
        move |_, offset_x, offset_y| continue_pan(
            &canvas,
            &contact,
            &last_offset,
            gesture,
            (offset_x, offset_y)
        )
    ));
    drag.connect_drag_end(glib::clone!(
        #[strong]
        contact,
        move |_, _, _| contact.pans.set(gesture, false)
    ));
    drag
}

fn begin_pan(
    canvas: &CadmiumCanvas,
    contact: &Contact,
    last_offset: &Cell<(f64, f64)>,
    drag: &gtk::GestureDrag,
    gesture: PanGesture,
) {
    let state = canvas.state();
    let press = state.borrow().ui_tool.profile().press_on(canvas.surface());
    let stroking = state.borrow().is_stroking();
    let wants_pan = gesture != PanGesture::Primary || press == Press::Pan;
    let pans = wants_pan && side_button_may_act(stroking);
    contact.pans.set(gesture, pans);
    last_offset.set((0.0, 0.0));
    if !pans {
        drag.set_state(gtk::EventSequenceState::Denied);
    }
}

fn continue_pan(
    canvas: &CadmiumCanvas,
    contact: &Contact,
    last_offset: &Cell<(f64, f64)>,
    gesture: PanGesture,
    offset: (f64, f64),
) {
    if !contact.pans.is_panning(gesture) {
        return;
    }
    let (offset_x, offset_y) = offset;
    let (last_x, last_y) = last_offset.replace((offset_x, offset_y));
    canvas.pan_by(offset_x - last_x, offset_y - last_y);
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
