use super::*;

#[test]
fn the_plot_is_a_square_centred_in_the_widget_inside_the_padding() {
    let plot = Plot::in_widget(300.0, 200.0);
    assert_eq!(
        plot.to_widget((0.0, 0.0)),
        (50.0 + PADDING, 200.0 - PADDING)
    );
    assert_eq!(plot.to_widget((1.0, 1.0)), (250.0 - PADDING, PADDING));
}

#[test]
fn a_widget_point_maps_back_to_the_unit_point_it_came_from() {
    let plot = Plot::in_widget(200.0, 200.0);
    let (x, y) = plot.to_widget((0.25, 0.75));
    let back = plot.to_unit(x, y);
    assert!((back.0 - 0.25).abs() < 1e-6 && (back.1 - 0.75).abs() < 1e-6);
}

#[test]
fn points_off_the_plot_land_on_its_edge() {
    let plot = Plot::in_widget(200.0, 200.0);
    assert_eq!(plot.to_unit(-50.0, 500.0), (0.0, 0.0));
    assert_eq!(plot.to_unit(500.0, -50.0), (1.0, 1.0));
}

#[test]
fn a_distance_in_widget_px_scales_with_the_plot() {
    let plot = Plot::in_widget(100.0 + 2.0 * PADDING, 100.0 + 2.0 * PADDING);
    assert!((plot.unit_distance(10.0) - 0.1).abs() < 1e-6);
}

#[test]
fn a_widget_too_small_for_the_padding_still_gives_a_usable_plot() {
    let plot = Plot::in_widget(4.0, 4.0);
    assert!(plot.unit_distance(1.0).is_finite());
}
