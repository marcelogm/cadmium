use super::*;

fn request(pressure: f32, seconds: f32) -> DrawUpRequest {
    DrawUpRequest {
        center: [0.0, 0.0],
        radius: 10.0,
        pressure,
        seconds,
    }
}

#[test]
fn share_grows_with_pressure_and_time() {
    let params = SimParams::default();
    let light = drawn_share(&params, &request(0.2, 0.05));
    assert!(drawn_share(&params, &request(1.0, 0.05)) > light);
    assert!(drawn_share(&params, &request(0.2, 0.1)) > light);
}

#[test]
fn share_never_exceeds_everything_under_the_brush() {
    let params = SimParams::default();
    assert_eq!(drawn_share(&params, &request(1.0, 1e6)), 1.0);
    assert_eq!(drawn_share(&params, &request(1.0, f32::INFINITY)), 1.0);
}
