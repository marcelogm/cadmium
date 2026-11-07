use super::*;

#[test]
fn only_points_on_the_tray_image_are_on_the_tray() {
    let (width, height) = (f64::from(TRAY_WIDTH), f64::from(TRAY_HEIGHT));
    for inside in [
        (0.0, 0.0),
        (width - 0.5, height - 0.5),
        (width / 2.0, height / 2.0),
    ] {
        assert!(is_on_tray(inside), "{inside:?}");
    }
    for outside in [
        (-0.5, 10.0),
        (10.0, -0.5),
        (width, 10.0),
        (10.0, height),
        (f64::NAN, 10.0),
    ] {
        assert!(!is_on_tray(outside), "{outside:?}");
    }
}

#[test]
fn a_short_widget_leaves_room_beside_the_tray() {
    let viewport = (384, 150);
    assert!(is_on_tray(tray_point(viewport, 1.0, 100.0, 100.0)));
    assert!(!is_on_tray(tray_point(viewport, 1.0, 383.0, 100.0)));
}

#[test]
fn a_tall_widget_leaves_room_below_the_tray() {
    let viewport = (384, 400);
    assert!(is_on_tray(tray_point(viewport, 1.0, 383.0, 191.0)));
    assert!(!is_on_tray(tray_point(viewport, 1.0, 100.0, 300.0)));
}

#[test]
fn the_widget_corners_are_the_tray_corners() {
    let viewport = (768, 384);
    assert_eq!(tray_point(viewport, 2.0, 0.0, 0.0), (0.0, 0.0));
    assert_eq!(
        tray_point(viewport, 2.0, 384.0, 192.0),
        (f64::from(TRAY_WIDTH), f64::from(TRAY_HEIGHT))
    );
}

#[test]
fn the_tray_is_half_as_tall_as_it_is_wide() {
    assert_eq!(tray_height_for_width(384), 192);
    assert_eq!(tray_height_for_width(TRAY_WIDTH as i32), TRAY_HEIGHT as i32);
}

#[test]
fn the_tray_image_fills_the_width_its_height_was_measured_for() {
    for width in [180, 316, 317, 331, 356] {
        let viewport = (width as u32, tray_height_for_width(width) as u32);
        let zoom = tray_view(viewport).zoom;
        assert_eq!(zoom, width as f32 / TRAY_WIDTH as f32, "{width}");
        assert!(TRAY_HEIGHT as f32 * zoom <= viewport.1 as f32, "{width}");
        assert!(
            viewport.1 as f32 - TRAY_HEIGHT as f32 * zoom < 1.0,
            "{width}"
        );
    }
}

#[test]
fn a_widget_pixel_covers_one_over_the_screen_zoom_tray_pixels() {
    for (viewport, scale) in [((384, 400), 1.0), ((600, 300), 1.5), ((768, 900), 2.0)] {
        let zoom = f64::from(tray_screen_zoom(viewport, scale));
        let (x0, y0) = tray_point(viewport, scale, 10.0, 20.0);
        let (x1, y1) = tray_point(viewport, scale, 11.0, 23.0);
        assert!(((x1 - x0) - 1.0 / zoom).abs() < 1e-3);
        assert!(((y1 - y0) - 3.0 / zoom).abs() < 1e-3);
    }
}
