use super::contain;

const CLOSE: f64 = 1e-9;

#[test]
fn a_wide_picture_fills_the_width_and_centres_vertically() {
    let fit = contain([0.0, 0.0], [240.0, 72.0], [153.0, 46.0], 0.0);
    assert!((fit.scale - 153.0 / 240.0).abs() < CLOSE);
    let (x, y) = fit.place([0.0, 0.0]);
    assert!(x.abs() < CLOSE);
    assert!((y - (46.0 - 72.0 * 153.0 / 240.0) / 2.0).abs() < CLOSE);
}

#[test]
fn a_tall_drawing_fills_the_height_inside_the_padding_and_centres_across() {
    let fit = contain([-1.0, -2.5], [1.0, 6.0], [30.0, 30.0], 2.0);
    assert!((fit.scale - 26.0 / 8.5).abs() < CLOSE);
    let (x, top) = fit.place([0.0, -2.5]);
    let (_, bottom) = fit.place([0.0, 6.0]);
    assert!((x - 15.0).abs() < CLOSE);
    assert!((top - 2.0).abs() < CLOSE);
    assert!((bottom - 28.0).abs() < CLOSE);
}

#[test]
fn an_empty_box_still_lands_somewhere_finite() {
    let fit = contain([1.0, 1.0], [1.0, 1.0], [30.0, 30.0], 2.0);
    let (x, y) = fit.place([1.0, 1.0]);
    assert!(fit.scale.is_finite() && x.is_finite() && y.is_finite());
}
