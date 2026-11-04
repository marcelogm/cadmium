use super::*;

fn sample_timed(time_ms: u32) -> StrokeSample {
    StrokeSample {
        x: 0.0,
        y: 0.0,
        pressure: 0.5,
        tilt_x_deg: 10.0,
        tilt_y_deg: -20.0,
        time_ms,
    }
}

#[test]
fn pen_rate_follows_the_sample_interval() {
    let mut readout = PenReadout::default();
    readout.record(&sample_timed(0), None);
    readout.record(&sample_timed(5), Some(0));
    assert!((readout.rate_hz - 200.0).abs() < 1e-3);
    assert_eq!(readout.tilt_y, -20.0);
}

#[test]
fn samples_with_the_same_timestamp_leave_the_rate_alone() {
    let mut readout = PenReadout {
        rate_hz: 120.0,
        ..PenReadout::default()
    };
    readout.record(&sample_timed(7), Some(7));
    assert_eq!(readout.rate_hz, 120.0);
}

#[test]
fn clock_going_backwards_does_not_spike_the_rate() {
    let mut readout = PenReadout::default();
    readout.record(&sample_timed(3), Some(10));
    assert_eq!(readout.rate_hz, 0.0);
}
