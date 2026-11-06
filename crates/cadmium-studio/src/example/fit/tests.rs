use cadmium_engine::brush::{BrushSpec, factory_brushes};

use super::{
    CANVAS_REACHES, EXAMPLE_SHEET_SIZE, ExampleFit, MARGIN_REACHES, MAX_CANVAS, canvas_for,
    reach_for,
};

fn sized(size_px: f32) -> Vec<BrushSpec> {
    factory_brushes()
        .into_iter()
        .map(|brush| BrushSpec { size_px, ..brush })
        .collect()
}

fn aspect((width, height): (u32, u32)) -> f64 {
    f64::from(width) / f64::from(height)
}

fn fits((width, height): (u32, u32)) -> bool {
    width <= MAX_CANVAS.0 && height <= MAX_CANVAS.1
}

#[test]
fn the_canvas_has_the_sheets_aspect() {
    let sheet = aspect(EXAMPLE_SHEET_SIZE);
    let reaches = CANVAS_REACHES.0 / CANVAS_REACHES.1;
    assert!(
        (reaches / sheet - 1.0).abs() < 0.005,
        "{reaches} vs {sheet}"
    );
    assert!((aspect(MAX_CANVAS) / sheet - 1.0).abs() < 0.005);
}

#[test]
fn a_small_brush_keeps_its_size() {
    for brush in sized(12.0) {
        let fit = ExampleFit::of(&brush);
        assert_eq!(fit.brush, brush);
        assert_eq!(fit.reach_px, reach_for(&brush, brush.size_px));
        assert_eq!(fit.canvas, canvas_for(fit.reach_px), "{}", brush.name);
    }
}

#[test]
fn every_factory_brush_at_its_suggested_size_fits() {
    for brush in factory_brushes() {
        let fit = ExampleFit::of(&brush);
        assert!(fits(fit.canvas), "{} {:?}", brush.name, fit.canvas);
        assert!(fit.brush.size_px <= brush.size_px, "{}", brush.name);
        assert_eq!(fit.brush.head, brush.head);
    }
}

#[test]
fn a_huge_brush_is_scaled_down_until_its_canvas_fits() {
    for brush in sized(BrushSpec::SIZE_RANGE_PX[1]) {
        assert!(!fits(canvas_for(reach_for(&brush, brush.size_px))));
        let fit = ExampleFit::of(&brush);
        assert!(fit.brush.size_px < brush.size_px, "{}", brush.name);
        assert_eq!(fit.reach_px, reach_for(&fit.brush, fit.brush.size_px));
        assert_eq!(fit.canvas, canvas_for(fit.reach_px));
        assert!(fits(fit.canvas), "{}", brush.name);
        let (width, height) = fit.canvas;
        let roomy = width * 10 > MAX_CANVAS.0 * 9 || height * 10 > MAX_CANVAS.1 * 9;
        assert!(roomy, "{} {:?}", brush.name, fit.canvas);
    }
}

#[test]
fn the_strokes_run_inside_the_margins_one_above_the_other() {
    let fit = ExampleFit::of(&sized(12.0)[0]);
    let reach = f64::from(fit.reach_px);
    let (curve, ramp) = (fit.s_curve_box(), fit.ramp_box());
    let (width, height) = (f64::from(fit.canvas.0), f64::from(fit.canvas.1));
    for area in [curve, ramp] {
        assert!((area.x - MARGIN_REACHES * reach).abs() < 1e-9);
        assert!(area.x + area.width + MARGIN_REACHES * reach <= width + 1e-9);
    }
    assert!((curve.y - MARGIN_REACHES * reach).abs() < 1e-9);
    assert!((curve.height - 2.0 * reach).abs() < 1e-9);
    assert!((ramp.y - (curve.y + curve.height) - 2.0 * reach).abs() < 1e-9);
    assert!((ramp.height - reach).abs() < 1e-9);
    assert!(ramp.y + ramp.height + MARGIN_REACHES * reach <= height + 1e-9);
}

#[test]
fn a_brush_of_no_real_size_gets_the_smallest_size() {
    for size_px in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let fit = ExampleFit::of(&sized(size_px)[0]);
        assert_eq!(fit.brush.size_px, BrushSpec::SIZE_RANGE_PX[0], "{size_px}");
        assert_eq!(fit.canvas, canvas_for(fit.reach_px));
    }
}
