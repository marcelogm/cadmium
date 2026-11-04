use super::*;
use cadmium_engine::params::SimParams;

fn spec_named(key: &str) -> &'static ParamSpec {
    SimParams::specs()
        .iter()
        .find(|spec| spec.key == key)
        .unwrap()
}

#[test]
fn param_values_show_as_many_decimals_as_their_slider_step() {
    assert_eq!(format_param(spec_named("bristle_count"), 24.0), "24");
    assert_eq!(format_param(spec_named("transfer_fraction"), 0.1), "0.10");
    assert_eq!(format_param(spec_named("residual_um"), 2.0), "2.0");
    assert_eq!(format_param(spec_named("penetration_um"), 120.0), "120");
}

#[test]
fn decimals_follow_the_step_size() {
    assert_eq!(decimals_for_step(0.01), 2);
    assert_eq!(decimals_for_step(0.02), 2);
    assert_eq!(decimals_for_step(0.1), 1);
    assert_eq!(decimals_for_step(0.2), 1);
    assert_eq!(decimals_for_step(1.0), 0);
    assert_eq!(decimals_for_step(3.0), 0);
}

#[test]
fn bristle_count_moves_in_whole_bristles() {
    assert_eq!(param_step(spec_named("bristle_count")), 1.0);
}
