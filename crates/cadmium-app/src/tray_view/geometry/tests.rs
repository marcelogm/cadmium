use super::*;
use crate::view::ViewStep;

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

fn assert_near((x, y): (f64, f64), (ex, ey): (f64, f64)) {
    assert!(
        (x - ex).abs() < 1e-3 && (y - ey).abs() < 1e-3,
        "{x},{y} vs {ex},{ey}"
    );
}

#[test]
fn the_tray_covers_a_wide_pane_edge_to_edge_cropping_top_and_bottom() {
    let viewport = (2000, 400);
    let view = tray_cover(viewport);
    // Every widget corner is on the tray, and the centres line up.
    for corner in [(0.0, 0.0), (1999.0, 0.0), (0.0, 399.0), (1999.0, 399.0)] {
        assert!(
            is_on_tray(tray_point(view, 1.0, corner.0, corner.1)),
            "{corner:?}"
        );
    }
    assert_near(
        tray_point(view, 1.0, 1000.0, 200.0),
        (f64::from(TRAY_WIDTH) / 2.0, f64::from(TRAY_HEIGHT) / 2.0),
    );
    // The width fits exactly; the height overflows evenly.
    assert_near(
        tray_point(view, 1.0, 0.0, 200.0),
        (0.0, f64::from(TRAY_HEIGHT) / 2.0),
    );
    let (_, top) = tray_point(view, 1.0, 1000.0, 0.0);
    let (_, bottom) = tray_point(view, 1.0, 1000.0, 400.0);
    assert!(top > 0.0 && bottom < f64::from(TRAY_HEIGHT));
    assert!((top - (f64::from(TRAY_HEIGHT) - bottom)).abs() < 1e-3);
}

#[test]
fn the_tray_covers_a_tall_pane_edge_to_edge_cropping_the_sides() {
    let viewport = (300, 600);
    let view = tray_cover(viewport);
    assert_near(
        tray_point(view, 1.0, 150.0, 300.0),
        (f64::from(TRAY_WIDTH) / 2.0, f64::from(TRAY_HEIGHT) / 2.0),
    );
    let (_, top) = tray_point(view, 1.0, 150.0, 0.0);
    let (_, bottom) = tray_point(view, 1.0, 150.0, 600.0);
    assert_near((top, bottom), (0.0, f64::from(TRAY_HEIGHT)));
    let (left, _) = tray_point(view, 1.0, 0.0, 300.0);
    let (right, _) = tray_point(view, 1.0, 300.0, 300.0);
    assert!(left > 0.0 && right < f64::from(TRAY_WIDTH));
}

#[test]
fn a_widget_point_maps_through_a_moved_view_and_back() {
    let moved = view::step(
        view::step(
            tray_cover((800, 400)),
            ViewStep::Pan {
                dx: -120.0,
                dy: 35.0,
            },
            2.0,
        ),
        ViewStep::Zoom {
            anchor: (90.0, 60.0),
            factor: 3.0,
        },
        2.0,
    );
    let scale = 2.0;
    for (x, y) in [(0.0, 0.0), (90.0, 60.0), (321.5, 177.25)] {
        let on_tray = tray_point(moved, scale, x, y);
        let (px, py) = view::canvas_to_viewport(moved, on_tray.0, on_tray.1);
        assert_near((px / scale, py / scale), (x, y));
    }
    // The zoom anchor stays over the same tray point.
    let before = tray_point(
        view::step(
            tray_cover((800, 400)),
            ViewStep::Pan {
                dx: -120.0,
                dy: 35.0,
            },
            2.0,
        ),
        scale,
        90.0,
        60.0,
    );
    assert_near(tray_point(moved, scale, 90.0, 60.0), before);
}

#[test]
fn zoomed_out_the_widget_shows_room_around_the_tray() {
    let view = view::step(
        tray_cover((800, 400)),
        ViewStep::Zoom {
            anchor: (400.0, 200.0),
            factor: 0.5,
        },
        1.0,
    );
    assert!(is_on_tray(tray_point(view, 1.0, 400.0, 200.0)));
    assert!(!is_on_tray(tray_point(view, 1.0, 5.0, 5.0)));
    assert!(!is_on_tray(tray_point(view, 1.0, 795.0, 395.0)));
}

#[test]
fn the_tray_asks_for_a_comfortable_height_and_settles_for_less() {
    let (minimum, natural) = tray_height_request();
    assert_eq!((minimum, natural), (TRAY_MIN_HEIGHT, TRAY_NATURAL_HEIGHT));
    assert!(minimum < natural);
}
