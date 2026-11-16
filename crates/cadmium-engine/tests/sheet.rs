pub mod support;

use cadmium_engine::{
    canvas::{Primer, TILE_SIZE, Weave},
    gpu::GpuContext,
    session::{ClearOutcome, PaintSession},
    sheet::{SHEET_HEIGHT_PX, SHEET_WIDTH_PX},
};
use support::{stroke, without_wgpu_errors};

const BLUE: usize = 0;

fn sheet(gpu: &GpuContext) -> PaintSession {
    let slots = support::spec(512, 256).pigment_slots;
    without_wgpu_errors(gpu, || PaintSession::for_sheet(gpu, &slots).unwrap())
}

fn mix_on(gpu: &GpuContext, sheet: &mut PaintSession) -> f64 {
    sheet.brush.recipe = vec![(BLUE, 1.0)];
    sheet.reload_brush(gpu);
    stroke(gpu, sheet, 128.0, 40.0, 470.0);
    let painted = sheet.canvas.wet_totals(gpu).unwrap().volume;
    assert!(painted > 0.0);
    painted
}

#[test]
fn a_sheet_is_a_small_plain_canvas_with_the_paintings_paints_and_a_clean_tool() {
    let gpu = GpuContext::new().unwrap();
    let sheet = sheet(&gpu);
    let spec = &sheet.canvas.spec;
    assert_eq!((spec.width, spec.height), (SHEET_WIDTH_PX, SHEET_HEIGHT_PX));
    assert_eq!((spec.primer, spec.weave), (Primer::Paper, Weave::Smooth));
    assert_eq!(spec.pigment_slots, support::spec(512, 256).pigment_slots);
    assert_eq!(sheet.engine.brush_totals(&gpu).unwrap().volume, 0.0);
    let tiles = SHEET_WIDTH_PX.div_ceil(TILE_SIZE) * SHEET_HEIGHT_PX.div_ceil(TILE_SIZE);
    assert_eq!(sheet.history.slot_capacity(), tiles * 4);
}

#[test]
fn a_stroke_on_a_sheet_can_be_undone() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = sheet(&gpu);
    mix_on(&gpu, &mut sheet);
    assert!(without_wgpu_errors(&gpu, || sheet.undo(&gpu)));
    assert_eq!(sheet.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn clearing_a_sheet_removes_its_wet_paint_in_one_undoable_step() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = sheet(&gpu);
    assert_eq!(sheet.clear_wet(&gpu), ClearOutcome::NothingToClear);
    let painted = mix_on(&gpu, &mut sheet);
    assert_eq!(
        sheet.clear_wet(&gpu),
        ClearOutcome::Cleared { undoable: true }
    );
    assert_eq!(sheet.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(sheet.undo(&gpu));
    assert_eq!(sheet.canvas.wet_totals(&gpu).unwrap().volume, painted);
}

#[test]
fn redoing_a_clear_leaves_nothing_to_clear() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = sheet(&gpu);
    mix_on(&gpu, &mut sheet);
    sheet.clear_wet(&gpu);
    assert!(sheet.undo(&gpu));
    assert!(sheet.redo(&gpu));
    assert_eq!(sheet.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
    assert_eq!(sheet.clear_wet(&gpu), ClearOutcome::NothingToClear);
}
