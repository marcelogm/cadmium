use std::cell::Cell;
use std::rc::Rc;

use gtk::glib::translate::IntoGlib;
use gtk::prelude::*;
use gtk::{gdk, glib};
use oleo_engine::stroke::{StrokeSample, Tool};

use crate::canvas_widget::OleoCanvas;
use crate::tools::UiTool;

const TILT_AXIS_TO_DEGREES: f64 = 90.0;
const MOUSE_PRESSURE: f32 = 1.0;
const ZOOM_PER_SCROLL_STEP: f32 = 1.1;
const SURFACE_SCROLL_PER_STEP: f64 = 15.0;
const WHEEL_PAN_STEP: f64 = 48.0;
const PAN_BUTTON: u32 = gdk::BUTTON_MIDDLE;
const EYEDROPPER_BUTTON: u32 = gdk::BUTTON_SECONDARY;

pub fn attach(canvas: &OleoCanvas) {
    canvas.add_controller(stylus_gesture(canvas));
    canvas.add_controller(pan_gesture(canvas, gdk::BUTTON_PRIMARY));
    canvas.add_controller(pan_gesture(canvas, PAN_BUTTON));
    canvas.add_controller(eyedropper_click(canvas));
    canvas.add_controller(scroll_controller(canvas));
    canvas.add_controller(hover_controller(canvas));
    canvas.add_controller(pinch_gesture(canvas));
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

fn event_sample(canvas: &OleoCanvas, event: &gdk::Event, x: f64, y: f64) -> StrokeSample {
    let (canvas_x, canvas_y) = canvas.canvas_point(x, y);
    StrokeSample {
        x: canvas_x,
        y: canvas_y,
        pressure: pressure_or(event.axis(gdk::AxisUse::Pressure), MOUSE_PRESSURE),
        tilt_x_deg: tilt_degrees_or(event.axis(gdk::AxisUse::Xtilt), 0.0),
        tilt_y_deg: tilt_degrees_or(event.axis(gdk::AxisUse::Ytilt), 0.0),
        time_ms: event.time(),
    }
}

fn motion_samples(canvas: &OleoCanvas, event: &gdk::Event, x: f64, y: f64) -> Vec<StrokeSample> {
    let current = event_sample(canvas, event, x, y);
    let history = event.history();
    let mut samples = Vec::with_capacity(history.len() + 1);
    if let Some((event_x, event_y)) = event.position() {
        let (offset_x, offset_y) = (x - event_x, y - event_y);
        let to_canvas = |hx: f64, hy: f64| canvas.canvas_point(hx + offset_x, hy + offset_y);
        samples.extend(
            history
                .iter()
                .map(|coord| history_sample(coord, to_canvas, &current)),
        );
    }
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

fn is_eraser(event: &gdk::Event) -> bool {
    event
        .device_tool()
        .is_some_and(|tool| tool.tool_type() == gdk::DeviceToolType::Eraser)
}

fn stylus_gesture(canvas: &OleoCanvas) -> gtk::GestureStylus {
    let stylus = gtk::GestureStylus::new();
    stylus.set_stylus_only(false);
    stylus.connect_down(glib::clone!(
        #[weak]
        canvas,
        move |stylus, x, y| {
            let Some(event) = stylus.current_event() else {
                return;
            };
            if !is_primary_contact(&event) {
                return;
            }
            let state = canvas.state();
            let active = state.borrow().ui_tool;
            let UiTool::Paint(tool) = active else {
                return;
            };
            let tool = if is_eraser(&event) { Tool::Rag } else { tool };
            if tool == Tool::Eyedropper {
                canvas.pick_colour_at(x, y);
                return;
            }
            let sample = event_sample(&canvas, &event, x, y);
            state.borrow_mut().begin_stroke(tool, sample);
            canvas.set_hover(Some((x, y)));
        }
    ));
    stylus.connect_motion(glib::clone!(
        #[weak]
        canvas,
        move |stylus, x, y| {
            canvas.set_hover(Some((x, y)));
            let state = canvas.state();
            if !state.borrow().is_stroking() {
                return;
            }
            let Some(event) = stylus.current_event() else {
                return;
            };
            let samples = motion_samples(&canvas, &event, x, y);
            state.borrow_mut().add_samples(&samples);
        }
    ));
    stylus.connect_up(glib::clone!(
        #[weak]
        canvas,
        move |stylus, _, _| {
            if stylus
                .current_event()
                .is_some_and(|event| is_primary_contact(&event))
            {
                canvas.state().borrow_mut().end_stroke();
            }
        }
    ));
    stylus.connect_proximity(glib::clone!(
        #[weak]
        canvas,
        move |stylus, x, y| {
            let leaving = stylus
                .current_event()
                .is_some_and(|event| event.event_type() == gdk::EventType::ProximityOut);
            if leaving {
                canvas.state().borrow_mut().end_stroke();
                canvas.set_hover(None);
            } else {
                canvas.set_hover(Some((x, y)));
            }
        }
    ));
    stylus.connect_cancel(glib::clone!(
        #[weak]
        canvas,
        move |_, _| canvas.state().borrow_mut().end_stroke()
    ));
    stylus
}

fn pan_gesture(canvas: &OleoCanvas, button: u32) -> gtk::GestureDrag {
    let drag = gtk::GestureDrag::new();
    drag.set_button(button);
    let panning = Rc::new(Cell::new(false));
    let last_offset = Rc::new(Cell::new((0.0, 0.0)));
    drag.connect_drag_begin(glib::clone!(
        #[weak]
        canvas,
        #[strong]
        panning,
        #[strong]
        last_offset,
        move |drag, _, _| {
            let moving = canvas.state().borrow().ui_tool == UiTool::Move;
            let pans = button != gdk::BUTTON_PRIMARY || moving;
            panning.set(pans);
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
        panning,
        move |_, offset_x, offset_y| {
            if !panning.get() {
                return;
            }
            let (last_x, last_y) = last_offset.replace((offset_x, offset_y));
            canvas.pan_by(offset_x - last_x, offset_y - last_y);
        }
    ));
    drag.connect_drag_end(move |_, _, _| panning.set(false));
    drag
}

fn eyedropper_click(canvas: &OleoCanvas) -> gtk::GestureClick {
    let click = gtk::GestureClick::new();
    click.set_button(EYEDROPPER_BUTTON);
    click.connect_pressed(glib::clone!(
        #[weak]
        canvas,
        move |_, _, x, y| canvas.pick_colour_at(x, y)
    ));
    click
}

fn scroll_steps(controller: &gtk::EventControllerScroll, delta: f64) -> f64 {
    match controller.unit() {
        gdk::ScrollUnit::Wheel => delta,
        _ => delta / SURFACE_SCROLL_PER_STEP,
    }
}

fn scroll_controller(canvas: &OleoCanvas) -> gtk::EventControllerScroll {
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

fn hover_controller(canvas: &OleoCanvas) -> gtk::EventControllerMotion {
    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter(glib::clone!(
        #[weak]
        canvas,
        move |_, x, y| canvas.set_hover(Some((x, y)))
    ));
    motion.connect_motion(glib::clone!(
        #[weak]
        canvas,
        move |_, x, y| canvas.set_hover(Some((x, y)))
    ));
    motion.connect_leave(glib::clone!(
        #[weak]
        canvas,
        move |_| {
            canvas.set_hover(None);
            canvas.state().borrow_mut().end_stroke();
        }
    ));
    motion
}

fn pinch_gesture(canvas: &OleoCanvas) -> gtk::GestureZoom {
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
    fn missing_pressure_axis_means_a_mouse_at_full_pressure() {
        assert_eq!(pressure_or(None, MOUSE_PRESSURE), 1.0);
        assert_eq!(pressure_or(Some(1.4), MOUSE_PRESSURE), 1.0);
    }
}
