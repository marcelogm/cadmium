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
fn missing_pressure_axis_means_a_mouse_at_full_pressure() {
    assert_eq!(pressure_or(None, MOUSE_PRESSURE), 1.0);
    assert_eq!(pressure_or(Some(1.4), MOUSE_PRESSURE), 1.0);
}

#[test]
fn a_history_point_where_the_event_happened_lands_where_the_event_landed() {
    let to_widget = SurfaceToWidget::anchored((300.0, 120.0), (40.0, 70.0));
    assert_eq!(to_widget.point(300.0, 120.0), (40.0, 70.0));
}

#[test]
fn a_history_point_keeps_its_distance_from_the_event() {
    let to_widget = SurfaceToWidget::anchored((300.0, 120.0), (40.0, 70.0));
    assert_eq!(to_widget.point(295.0, 132.0), (35.0, 82.0));
}
