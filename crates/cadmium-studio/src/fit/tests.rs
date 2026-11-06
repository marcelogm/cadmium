use super::{BOX_REACHES, MARGIN_REACHES, MAX_CANVAS, StudioFit, canvas_for, reach_for};
use cadmium_engine::brush::{BrushSpec, factory_brushes};

fn sized(size_px: f32) -> Vec<BrushSpec> {
    factory_brushes()
        .into_iter()
        .map(|brush| BrushSpec { size_px, ..brush })
        .collect()
}

#[test]
fn a_small_brush_gets_ten_by_six_reaches_unchanged() {
    for brush in sized(8.0) {
        let fit = StudioFit::of(&brush);
        assert_eq!(fit.brush, brush);
        let reach = reach_for(&brush, brush.size_px);
        assert_eq!(fit.reach_px, reach);
        let reach = f64::from(reach);
        let want = ((10.0 * reach).ceil() as u32, (6.0 * reach).ceil() as u32);
        assert_eq!(fit.canvas, want, "{}", brush.name);
    }
}

#[test]
fn a_huge_brush_is_scaled_down_until_its_canvas_fits() {
    for brush in sized(BrushSpec::SIZE_RANGE_PX[1]) {
        assert!(canvas_for(reach_for(&brush, brush.size_px)).0 > MAX_CANVAS.0);
        let fit = StudioFit::of(&brush);
        assert!(fit.brush.size_px < brush.size_px, "{}", brush.name);
        assert_eq!(fit.brush.head, brush.head);
        assert_eq!(fit.reach_px, reach_for(&fit.brush, fit.brush.size_px));
        assert_eq!(fit.canvas, canvas_for(fit.reach_px));
        let (width, height) = fit.canvas;
        assert!(
            width <= MAX_CANVAS.0 && height <= MAX_CANVAS.1,
            "{}",
            brush.name
        );
        let roomy = width * 10 > MAX_CANVAS.0 * 9 || height * 10 > MAX_CANVAS.1 * 9;
        assert!(roomy, "{} {:?}", brush.name, fit.canvas);
    }
}

#[test]
fn the_stroke_box_sits_two_reaches_in_and_spans_six_by_two() {
    let fit = StudioFit::of(&sized(20.0)[0]);
    let reach = f64::from(fit.reach_px);
    let area = fit.stroke_box();
    assert_eq!(
        (area.x, area.y),
        (MARGIN_REACHES * reach, MARGIN_REACHES * reach)
    );
    assert_eq!(
        (area.width, area.height),
        (BOX_REACHES.0 * reach, BOX_REACHES.1 * reach)
    );
    assert!(area.x + area.width + MARGIN_REACHES * reach <= f64::from(fit.canvas.0));
    assert!(area.y + area.height + MARGIN_REACHES * reach <= f64::from(fit.canvas.1));
}

#[test]
fn a_brush_of_no_real_size_gets_the_smallest_size() {
    for size_px in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let fit = StudioFit::of(&sized(size_px)[0]);
        assert_eq!(fit.brush.size_px, BrushSpec::SIZE_RANGE_PX[0], "{size_px}");
        assert_eq!(fit.canvas, canvas_for(fit.reach_px));
    }
}
