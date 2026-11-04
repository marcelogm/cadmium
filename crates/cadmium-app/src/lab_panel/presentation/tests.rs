use super::*;
use cadmium_engine::params::SimParams;

#[test]
fn every_view_mode_round_trips_through_its_toggle_name() {
    for mode in [
        ViewMode::Color,
        ViewMode::Height,
        ViewMode::Wetness,
        ViewMode::Pigment(3),
    ] {
        assert_eq!(view_from_name(view_name(mode), 3), Some(mode));
    }
    assert_eq!(view_from_name("ultraviolet", 0), None);
}

#[test]
fn every_param_group_has_a_panel_title() {
    for spec in SimParams::specs() {
        assert!(
            PARAM_GROUPS.iter().any(|(group, _)| *group == spec.group),
            "{}",
            spec.key
        );
    }
}

#[test]
fn pen_readout_reads_as_pressure_tilt_samples_and_rate() {
    let pen = PenReadout {
        pressure: 0.5,
        tilt_x: 12.4,
        tilt_y: -30.0,
        samples_last_frame: 7,
        rate_hz: 199.6,
    };
    assert_eq!(pen_texts(&pen), ["0.50", "12° / -30°", "7", "200 Hz"]);
}
