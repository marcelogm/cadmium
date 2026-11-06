use cadmium_engine::brush::{BrushSpec, HeadSpec, factory_brushes};
use cadmium_engine::canvas::{Primer, Weave};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::params::SimParams;
use cadmium_engine::present::{ViewTransform, readback_rgba8};
use cadmium_engine::stroke::StrokeSample;

use super::{SheetFrame, TestSheet};
use crate::paint::{DEMO_PIGMENT, StudioPaint};

/// Small, so the tests stay quick on lavapipe.
const SIZE: (u32, u32) = (256, 192);
const BRUSH_PX: f32 = 12.0;
const FRAMES: u32 = 20;
const FRAME_STEP_MS: f64 = 60.0;
const ONE_TO_ONE: ViewTransform = ViewTransform {
    origin_x: 0.0,
    origin_y: 0.0,
    zoom: 1.0,
};
/// How far a fresh dip may differ from the first one, as a share of it.
const DIP_TOLERANCE: f64 = 0.01;
/// The darkest a bare paper pixel may present at (the ground alone is ≈ 247, 248, 244).
const PAPER_FLOOR: u8 = 238;

fn paint() -> StudioPaint {
    StudioPaint {
        slots: vec!["titanium_white".into(), DEMO_PIGMENT.into()],
        primer: Primer::WhiteGesso,
        weave: Weave::Smooth,
        recipe: vec![(1, 1.0)],
        load: 1.0,
        medium: 0.1,
        params: SimParams::default(),
    }
}

fn brush(head: HeadSpec) -> BrushSpec {
    let brush = factory_brushes()
        .into_iter()
        .find(|b| b.head == head)
        .unwrap();
    BrushSpec {
        size_px: BRUSH_PX,
        ..brush
    }
}

fn round_sheet(gpu: &GpuContext) -> TestSheet {
    TestSheet::new(gpu, &paint(), &brush(HeadSpec::ROUND), SIZE).unwrap()
}

/// A straight row at `y`, left to right, 8 ms apart.
fn row(y: f64) -> Vec<StrokeSample> {
    (0..20)
        .map(|i| StrokeSample {
            x: 40.0 + f64::from(i) * 8.0,
            y,
            pressure: 1.0,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            time_ms: i * 8,
        })
        .collect()
}

fn stroke(gpu: &GpuContext, sheet: &mut TestSheet, samples: &[StrokeSample]) {
    sheet.begin_stroke(gpu, samples[0]);
    sheet.add_samples(&samples[1..]);
    sheet.end_stroke();
}

/// Presents frames 60 ms apart from `clock_ms` on, so the sheet paints and settles; the
/// last frame's pixels.
fn shown(gpu: &GpuContext, sheet: &mut TestSheet, clock_ms: &mut f64) -> Vec<u8> {
    let target = sheet.session.presenter.create_target(gpu, SIZE.0, SIZE.1);
    for _ in 0..FRAMES {
        let frame = SheetFrame {
            clock_ms: *clock_ms,
            view: ONE_TO_ONE,
        };
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        sheet.encode_present(gpu, frame, &target, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        *clock_ms += FRAME_STEP_MS;
    }
    readback_rgba8(gpu, &target).unwrap()
}

fn wet_volume(gpu: &GpuContext, sheet: &TestSheet) -> f64 {
    sheet.session.canvas.wet_totals(gpu).unwrap().volume
}

fn reserve(gpu: &GpuContext, sheet: &mut TestSheet) -> f64 {
    sheet.session.read_brush(gpu).unwrap().reserve.volume
}

#[test]
fn a_sheet_is_plain_paper_of_the_size_asked_for_with_the_paintings_slots() {
    let gpu = GpuContext::new().unwrap();
    let linen = StudioPaint {
        weave: Weave::Linen,
        ..paint()
    };
    let sheet = TestSheet::new(&gpu, &linen, &brush(HeadSpec::ROUND), SIZE).unwrap();
    assert_eq!(sheet.size(), SIZE);
    let spec = &sheet.session.canvas.spec;
    assert_eq!(spec.pigment_slots, paint().slots);
    assert_eq!((spec.primer, spec.weave), (Primer::Paper, Weave::Smooth));
}

#[test]
fn a_stroke_changes_the_sheet() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = round_sheet(&gpu);
    let mut clock_ms = 0.0;
    let blank = shown(&gpu, &mut sheet, &mut clock_ms);
    assert!(
        blank[..3].iter().all(|c| *c >= PAPER_FLOOR),
        "{:?}",
        &blank[..3]
    );
    stroke(&gpu, &mut sheet, &row(60.0));
    let painted = shown(&gpu, &mut sheet, &mut clock_ms);
    assert!(painted != blank, "the stroke left no mark");
    assert!(wet_volume(&gpu, &sheet) > 0.0);
}

#[test]
fn clearing_gives_back_a_blank_sheet() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = round_sheet(&gpu);
    let mut clock_ms = 0.0;
    let blank = shown(&gpu, &mut sheet, &mut clock_ms);
    stroke(&gpu, &mut sheet, &row(60.0));
    shown(&gpu, &mut sheet, &mut clock_ms);
    sheet.clear(&gpu).unwrap();
    let cleared = shown(&gpu, &mut sheet, &mut clock_ms);
    assert!(cleared == blank, "the cleared sheet is not blank");
    assert_eq!(wet_volume(&gpu, &sheet), 0.0);
    assert_eq!(sheet.size(), SIZE);
}

#[test]
fn a_new_brush_waits_for_the_next_stroke_and_keeps_the_paint() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = round_sheet(&gpu);
    let mut clock_ms = 0.0;
    stroke(&gpu, &mut sheet, &row(60.0));
    shown(&gpu, &mut sheet, &mut clock_ms);
    let painted = wet_volume(&gpu, &sheet);
    let fan = brush(HeadSpec::FAN);
    sheet.set_brush(&fan);
    assert_eq!(wet_volume(&gpu, &sheet), painted);
    assert_eq!(sheet.session.brush.head, HeadSpec::ROUND);
    sheet.begin_stroke(&gpu, row(140.0)[0]);
    assert_eq!(sheet.session.brush.head, fan.head);
    assert_eq!(sheet.session.brush.geometry.max_diameter_px, BRUSH_PX);
}

#[test]
fn clearing_keeps_the_brush_for_the_next_stroke() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = round_sheet(&gpu);
    let fan = brush(HeadSpec::FAN);
    sheet.set_brush(&fan);
    sheet.clear(&gpu).unwrap();
    sheet.begin_stroke(&gpu, row(60.0)[0]);
    assert_eq!(sheet.session.brush.head, fan.head);
}

#[test]
fn every_stroke_starts_with_a_fresh_dip() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = round_sheet(&gpu);
    let mut clock_ms = 0.0;
    let first = row(60.0);
    sheet.begin_stroke(&gpu, first[0]);
    let dipped = reserve(&gpu, &mut sheet);
    sheet.add_samples(&first[1..]);
    sheet.end_stroke();
    shown(&gpu, &mut sheet, &mut clock_ms);
    let spent = reserve(&gpu, &mut sheet);
    sheet.begin_stroke(&gpu, row(140.0)[0]);
    let dipped_again = reserve(&gpu, &mut sheet);
    assert!(
        spent < dipped,
        "the stroke spent nothing: {spent} vs {dipped}"
    );
    assert!(
        (dipped_again - dipped).abs() <= dipped * DIP_TOLERANCE,
        "the second stroke began at {dipped_again}, the first at {dipped}"
    );
}
