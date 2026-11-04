use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;
use gtk::glib::translate::IntoGlib;
use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::canvas_widget::CadmiumCanvas;
use crate::state::SharedState;
use crate::tools::UiTool;

const TILT_AXIS_TO_DEGREES: f64 = 90.0;
const MOUSE_PRESSURE: f32 = 1.0;
const ZOOM_PER_SCROLL_STEP: f32 = 1.1;
const SURFACE_SCROLL_PER_STEP: f64 = 15.0;
const WHEEL_PAN_STEP: f64 = 48.0;
const PAN_BUTTON: u32 = gdk::BUTTON_MIDDLE;
const EYEDROPPER_BUTTON: u32 = gdk::BUTTON_SECONDARY;

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

fn axis_index(axis: gdk::AxisUse) -> usize {
    axis.into_glib() as usize
}

fn pressure_or(value: Option<f64>, fallback: f32) -> f32 {
    value.map_or(fallback, |pressure| pressure.clamp(0.0, 1.0) as f32)
}

fn tilt_degrees_or(value: Option<f64>, fallback: f32) -> f32 {
    value.map_or(fallback, |tilt| (tilt * TILT_AXIS_TO_DEGREES) as f32)
}

pub fn history_sample(
    coord: &gdk::TimeCoord,
    to_canvas: impl Fn(f64, f64) -> (f64, f64),
    fallback: &StrokeSample,
) -> StrokeSample {
    let axes = coord.axes();
    let flags = coord.flags();
    let axis = |axis: gdk::AxisUse, flag: gdk::AxisFlags| {
        flags.contains(flag).then(|| axes[axis_index(axis)])
    };
    let (x, y) = to_canvas(
        axes[axis_index(gdk::AxisUse::X)],
        axes[axis_index(gdk::AxisUse::Y)],
    );
    StrokeSample {
        x,
        y,
        pressure: pressure_or(
            axis(gdk::AxisUse::Pressure, gdk::AxisFlags::PRESSURE),
            fallback.pressure,
        ),
        tilt_x_deg: tilt_degrees_or(
            axis(gdk::AxisUse::Xtilt, gdk::AxisFlags::XTILT),
            fallback.tilt_x_deg,
        ),
        tilt_y_deg: tilt_degrees_or(
            axis(gdk::AxisUse::Ytilt, gdk::AxisFlags::YTILT),
            fallback.tilt_y_deg,
        ),
        time_ms: coord.time(),
    }
}

fn event_sample<W: PaintSurfaceWidget>(
    widget: &W,
    event: &gdk::Event,
    x: f64,
    y: f64,
) -> StrokeSample {
    let (surface_x, surface_y) = widget.surface_point(x, y);
    StrokeSample {
        x: surface_x,
        y: surface_y,
        pressure: pressure_or(event.axis(gdk::AxisUse::Pressure), MOUSE_PRESSURE),
        tilt_x_deg: tilt_degrees_or(event.axis(gdk::AxisUse::Xtilt), 0.0),
        tilt_y_deg: tilt_degrees_or(event.axis(gdk::AxisUse::Ytilt), 0.0),
        time_ms: event.time(),
    }
}

fn motion_samples<W: PaintSurfaceWidget>(
    widget: &W,
    stylus: &gtk::GestureStylus,
    event: &gdk::Event,
    x: f64,
    y: f64,
) -> Vec<StrokeSample> {
    let current = event_sample(widget, event, x, y);
    let backlog = stylus.backlog().unwrap_or_default();
    let mut samples = Vec::with_capacity(backlog.len() + 1);
    samples.extend(backlog.iter().map(|coord| {
        history_sample(
            coord,
            |widget_x, widget_y| widget.surface_point(widget_x, widget_y),
            &current,
        )
    }));
    samples.push(current);
    samples
}

fn is_primary_contact(event: &gdk::Event) -> bool {
    match event.downcast_ref::<gdk::ButtonEvent>() {
        Some(button) => button.button() == gdk::BUTTON_PRIMARY,
        None => matches!(
            event.event_type(),
            gdk::EventType::TouchBegin | gdk::EventType::TouchEnd | gdk::EventType::TouchCancel
        ),
    }
}

fn comes_from_a_pen(event: &gdk::Event) -> bool {
    event.device_tool().is_some()
}

fn is_eraser(event: &gdk::Event) -> bool {
    event
        .device_tool()
        .is_some_and(|tool| tool.tool_type() == gdk::DeviceToolType::Eraser)
}

fn stylus_gesture<W: PaintSurfaceWidget>(widget: &W, contact: &Rc<Contact>) -> gtk::GestureStylus {
    let stylus = gtk::GestureStylus::new();
    stylus.set_stylus_only(false);
    stylus.connect_down(glib::clone!(
        #[weak]
        widget,
        #[strong]
        contact,
        move |stylus, x, y| {
            let Some(event) = stylus.current_event() else {
                return;
            };
            if !is_primary_contact(&event) || !stroke_may_begin(contact.pans.any()) {
                return;
            }
            let state = widget.state();
            let active = state.borrow().ui_tool;
            let eraser = is_eraser(&event);
            if active == UiTool::Move {
                return;
            }
            if !eraser && active == UiTool::Paint(Tool::Eyedropper) {
                widget.pick_colour_at(x, y);
                return;
            }
            let sample = event_sample(&widget, &event, x, y);
            if !state.borrow_mut().press(widget.surface(), sample, eraser) {
                return;
            }
            contact
                .stroking_device
                .replace(stylus.current_event_device());
            widget.set_hover(Some((x, y)));
        }
    ));
    stylus.connect_motion(glib::clone!(
        #[weak]
        widget,
        #[strong]
        contact,
        move |stylus, x, y| {
            widget.set_hover(Some((x, y)));
            let Some(event) = stylus.current_event() else {
                return;
            };
            let state = widget.state();
            if !state.borrow().contact_on(widget.surface()) {
                if comes_from_a_pen(&event) {
                    widget.observe_hover_pen(&event_sample(&widget, &event, x, y));
                }
                return;
            }
            if !contact.is_stroking_device(stylus.current_event_device().as_ref()) {
                return;
            }
            let samples = motion_samples(&widget, stylus, &event, x, y);
            state.borrow_mut().motion(&samples);
        }
    ));
    stylus.connect_up(glib::clone!(
        #[weak]
        widget,
        #[strong]
        contact,
        move |stylus, _, _| {
            if stylus
                .current_event()
                .is_some_and(|event| is_primary_contact(&event))
            {
                release(&widget, &contact);
            }
        }
    ));
    stylus.connect_proximity(glib::clone!(
        #[weak]
        widget,
        #[strong]
        contact,
        move |stylus, x, y| {
            let Some(event) = stylus.current_event() else {
                return;
            };
            if event.event_type() == gdk::EventType::ProximityOut {
                if contact.is_stroking_device(stylus.current_event_device().as_ref()) {
                    release(&widget, &contact);
                }
                widget.set_hover(None);
                return;
            }
            widget.set_hover(Some((x, y)));
            if comes_from_a_pen(&event) {
                widget.observe_hover_pen(&event_sample(&widget, &event, x, y));
            }
        }
    ));
    stylus.connect_cancel(glib::clone!(
        #[weak]
        widget,
        #[strong]
        contact,
        move |_, _| release(&widget, &contact)
    ));
    stylus
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

fn pan_gesture(
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
        move |drag, _, _| {
            let (ui_tool, stroking) = {
                let state = canvas.state();
                let state = state.borrow();
                (state.ui_tool, state.is_stroking())
            };
            let wants_pan = gesture != PanGesture::Primary || ui_tool == UiTool::Move;
            let pans = wants_pan && side_button_may_act(stroking);
            contact.pans.set(gesture, pans);
            last_offset.set((0.0, 0.0));
            if !pans {
                drag.set_state(gtk::EventSequenceState::Denied);
            }
        }
    ));
    drag.connect_drag_update(glib::clone!(
        #[weak]
        canvas,
        #[strong]
        contact,
        move |_, offset_x, offset_y| {
            if !contact.pans.is_panning(gesture) {
                return;
            }
            let (last_x, last_y) = last_offset.replace((offset_x, offset_y));
            canvas.pan_by(offset_x - last_x, offset_y - last_y);
        }
    ));
    drag.connect_drag_end(glib::clone!(
        #[strong]
        contact,
        move |_, _, _| contact.pans.set(gesture, false)
    ));
    drag
}

fn eyedropper_click<W: PaintSurfaceWidget>(widget: &W) -> gtk::GestureClick {
    let click = gtk::GestureClick::new();
    click.set_button(EYEDROPPER_BUTTON);
    click.connect_pressed(glib::clone!(
        #[weak]
        widget,
        move |_, _, x, y| {
            if side_button_may_act(widget.state().borrow().is_stroking()) {
                widget.pick_colour_at(x, y);
            }
        }
    ));
    click
}

fn scroll_steps(controller: &gtk::EventControllerScroll, delta: f64) -> f64 {
    match controller.unit() {
        gdk::ScrollUnit::Wheel => delta,
        _ => delta / SURFACE_SCROLL_PER_STEP,
    }
}

fn scroll_controller(canvas: &CadmiumCanvas) -> gtk::EventControllerScroll {
    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    scroll.connect_scroll(glib::clone!(
        #[weak]
        canvas,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |controller, dx, dy| {
            let shift = controller
                .current_event_state()
                .contains(gdk::ModifierType::SHIFT_MASK);
            if shift {
                let step = match controller.unit() {
                    gdk::ScrollUnit::Wheel => WHEEL_PAN_STEP,
                    _ => 1.0,
                };
                let horizontal = if dx == 0.0 { dy } else { dx };
                let vertical = if dx == 0.0 { 0.0 } else { dy };
                canvas.pan_by(-horizontal * step, -vertical * step);
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
    ));
    scroll
}

fn hover_controller<W: PaintSurfaceWidget>(
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
        move |motion| {
            widget.set_hover(None);
            if contact.is_stroking_device(motion.current_event_device().as_ref()) {
                release(&widget, &contact);
            }
        }
    ));
    motion
}

fn pinch_gesture(canvas: &CadmiumCanvas) -> gtk::GestureZoom {
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
        move |pinch, scale| {
            let previous = last_scale.replace(scale);
            if previous <= 0.0 || scale <= 0.0 {
                return;
            }
            if let Some((x, y)) = pinch.bounding_box_center() {
                canvas.set_zoom_around(x, y, canvas.zoom() * (scale / previous) as f32);
            }
        }
    ));
    pinch
}

#[cfg(test)]
mod tests {
    use super::*;

    const AXIS_COUNT: usize = 12;

    fn fallback() -> StrokeSample {
        StrokeSample {
            x: 0.0,
            y: 0.0,
            pressure: 0.7,
            tilt_x_deg: 5.0,
            tilt_y_deg: -5.0,
            time_ms: 100,
        }
    }

    fn axes_with(values: &[(gdk::AxisUse, f64)]) -> [f64; AXIS_COUNT] {
        let mut axes = [0.0; AXIS_COUNT];
        for (axis, value) in values {
            axes[axis_index(*axis)] = *value;
        }
        axes
    }

    #[test]
    fn backlog_entry_reads_pressure_and_tilt_in_degrees() {
        let axes = axes_with(&[
            (gdk::AxisUse::X, 10.0),
            (gdk::AxisUse::Y, 20.0),
            (gdk::AxisUse::Pressure, 0.25),
            (gdk::AxisUse::Xtilt, 0.5),
            (gdk::AxisUse::Ytilt, -0.25),
        ]);
        let flags = gdk::AxisFlags::X
            | gdk::AxisFlags::Y
            | gdk::AxisFlags::PRESSURE
            | gdk::AxisFlags::XTILT
            | gdk::AxisFlags::YTILT;
        let coord = gdk::TimeCoord::new(90, axes, flags);
        let sample = history_sample(&coord, |x, y| (x * 2.0, y + 1.0), &fallback());
        assert_eq!((sample.x, sample.y), (20.0, 21.0));
        assert_eq!(sample.pressure, 0.25);
        assert_eq!(sample.tilt_x_deg, 45.0);
        assert_eq!(sample.tilt_y_deg, -22.5);
        assert_eq!(sample.time_ms, 90);
    }

    #[test]
    fn backlog_entry_without_pen_axes_keeps_the_current_pressure_and_tilt() {
        let axes = axes_with(&[
            (gdk::AxisUse::X, 3.0),
            (gdk::AxisUse::Y, 4.0),
            (gdk::AxisUse::Pressure, 0.9),
        ]);
        let coord = gdk::TimeCoord::new(95, axes, gdk::AxisFlags::X | gdk::AxisFlags::Y);
        let sample = history_sample(&coord, |x, y| (x, y), &fallback());
        assert_eq!((sample.x, sample.y), (3.0, 4.0));
        assert_eq!(sample.pressure, 0.7);
        assert_eq!(sample.tilt_x_deg, 5.0);
        assert_eq!(sample.tilt_y_deg, -5.0);
    }

    #[test]
    fn backlog_entries_are_widget_points_mapped_like_the_motion_event() {
        let axes = axes_with(&[(gdk::AxisUse::X, 12.0), (gdk::AxisUse::Y, 34.0)]);
        let coord = gdk::TimeCoord::new(80, axes, gdk::AxisFlags::X | gdk::AxisFlags::Y);
        let widget_to_canvas = |x: f64, y: f64| (x / 2.0 + 100.0, y / 2.0 + 50.0);
        let sample = history_sample(&coord, widget_to_canvas, &fallback());
        assert_eq!((sample.x, sample.y), widget_to_canvas(12.0, 34.0));
    }

    #[test]
    fn side_buttons_do_nothing_while_the_pen_is_painting() {
        assert!(!side_button_may_act(true));
        assert!(side_button_may_act(false));
    }

    #[test]
    fn pen_down_during_a_pan_does_not_paint() {
        assert!(!stroke_may_begin(true));
        assert!(stroke_may_begin(false));
    }

    #[test]
    fn one_pan_ending_leaves_the_other_panning() {
        let pans = PanFlags::default();
        pans.set(PanGesture::Primary, true);
        pans.set(PanGesture::Middle, true);
        pans.set(PanGesture::Middle, false);
        assert!(pans.any());
        assert!(pans.is_panning(PanGesture::Primary));
        assert!(!pans.is_panning(PanGesture::Middle));
        pans.set(PanGesture::Primary, false);
        assert!(!pans.any());
    }

    #[test]
    fn another_device_leaving_does_not_end_the_stroke() {
        let (pen, mouse) = ("pen", "mouse");
        assert!(!event_belongs_to_stroke(Some(&pen), Some(&mouse)));
        assert!(event_belongs_to_stroke(Some(&pen), Some(&pen)));
        assert!(event_belongs_to_stroke(Some(&pen), None));
        assert!(event_belongs_to_stroke(None, Some(&mouse)));
    }

    #[test]
    fn missing_pressure_axis_means_a_mouse_at_full_pressure() {
        assert_eq!(pressure_or(None, MOUSE_PRESSURE), 1.0);
        assert_eq!(pressure_or(Some(1.4), MOUSE_PRESSURE), 1.0);
    }
}
