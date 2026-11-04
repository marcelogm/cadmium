use super::*;

#[test]
fn tray_view_fits_the_whole_tray_in_the_widget() {
    let half = (TRAY_WIDTH / 2, TRAY_HEIGHT / 2);
    assert_eq!(tray_view((half.0, TRAY_HEIGHT)).zoom, 0.5);
    assert_eq!(tray_view((TRAY_WIDTH, half.1)).zoom, 0.5);
    assert_eq!(
        crate::view::viewport_to_canvas(tray_view(half), f64::from(half.0), f64::from(half.1)),
        (f64::from(TRAY_WIDTH), f64::from(TRAY_HEIGHT))
    );
}
