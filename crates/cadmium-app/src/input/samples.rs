use cadmium_engine::stroke::StrokeSample;
use gtk::gdk;
use gtk::glib::translate::IntoGlib;

use super::PaintSurfaceWidget;

const TILT_AXIS_TO_DEGREES: f64 = 90.0;
const MOUSE_PRESSURE: f32 = 1.0;

fn axis_index(axis: gdk::AxisUse) -> usize {
    axis.into_glib() as usize
}

fn pressure_or(value: Option<f64>, fallback: f32) -> f32 {
    value.map_or(fallback, |pressure| pressure.clamp(0.0, 1.0) as f32)
}

fn tilt_degrees_or(value: Option<f64>, fallback: f32) -> f32 {
    value.map_or(fallback, |tilt| (tilt * TILT_AXIS_TO_DEGREES) as f32)
}

fn history_sample(
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

pub(super) fn event_sample<W: PaintSurfaceWidget>(
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

pub(super) fn motion_samples<W: PaintSurfaceWidget>(
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

pub(super) fn is_primary_contact(event: &gdk::Event) -> bool {
    match event.downcast_ref::<gdk::ButtonEvent>() {
        Some(button) => button.button() == gdk::BUTTON_PRIMARY,
        None => matches!(
            event.event_type(),
            gdk::EventType::TouchBegin | gdk::EventType::TouchEnd | gdk::EventType::TouchCancel
        ),
    }
}

pub(super) fn comes_from_a_pen(event: &gdk::Event) -> bool {
    event.device_tool().is_some()
}

pub(super) fn is_eraser(event: &gdk::Event) -> bool {
    event
        .device_tool()
        .is_some_and(|tool| tool.tool_type() == gdk::DeviceToolType::Eraser)
}

#[cfg(test)]
mod tests;
