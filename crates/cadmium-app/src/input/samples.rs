use cadmium_engine::stroke::StrokeSample;
use gtk::gdk;
use gtk::glib::translate::IntoGlib;

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

struct SurfaceToWidget {
    offset: (f64, f64),
}

impl SurfaceToWidget {
    fn anchored(event_on_surface: (f64, f64), event_on_widget: (f64, f64)) -> Self {
        SurfaceToWidget {
            offset: (
                event_on_widget.0 - event_on_surface.0,
                event_on_widget.1 - event_on_surface.1,
            ),
        }
    }

    fn point(&self, surface_x: f64, surface_y: f64) -> (f64, f64) {
        (surface_x + self.offset.0, surface_y + self.offset.1)
    }
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

/// The pen's state at widget point (`x`, `y`), the point mapped by `to_canvas` onto what
/// is painted.
pub(crate) fn event_sample(
    event: &gdk::Event,
    x: f64,
    y: f64,
    to_canvas: impl Fn(f64, f64) -> (f64, f64),
) -> StrokeSample {
    let (canvas_x, canvas_y) = to_canvas(x, y);
    StrokeSample {
        x: canvas_x,
        y: canvas_y,
        pressure: pressure_or(event.axis(gdk::AxisUse::Pressure), MOUSE_PRESSURE),
        tilt_x_deg: tilt_degrees_or(event.axis(gdk::AxisUse::Xtilt), 0.0),
        tilt_y_deg: tilt_degrees_or(event.axis(gdk::AxisUse::Ytilt), 0.0),
        time_ms: event.time(),
    }
}

/// The motion's samples, oldest first: the event's history, then the event itself, each
/// mapped by `to_canvas` from widget points.
pub(crate) fn motion_samples(
    event: &gdk::Event,
    x: f64,
    y: f64,
    to_canvas: impl Fn(f64, f64) -> (f64, f64),
) -> Vec<StrokeSample> {
    let current = event_sample(event, x, y, &to_canvas);
    let Some(event_position) = event.position() else {
        return vec![current];
    };
    let to_widget = SurfaceToWidget::anchored(event_position, (x, y));
    let history = event.history();
    let mut samples = Vec::with_capacity(history.len() + 1);
    samples.extend(history.iter().map(|coord| {
        history_sample(
            coord,
            |surface_x, surface_y| {
                let (widget_x, widget_y) = to_widget.point(surface_x, surface_y);
                to_canvas(widget_x, widget_y)
            },
            &current,
        )
    }));
    samples.push(current);
    samples
}

pub(crate) fn is_primary_contact(event: &gdk::Event) -> bool {
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
