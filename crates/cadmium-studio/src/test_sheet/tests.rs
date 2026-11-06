use cadmium_engine::brush::{BrushSpec, HeadSpec, factory_brushes};
use cadmium_engine::canvas::{Primer, StateTexture, Weave};
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
    rotation: 0.0,
};
/// How far a fresh dip may differ from the first one, as a share of it.
const DIP_TOLERANCE: f64 = 0.01;
/// The darkest a bare paper pixel may present at (the ground alone is ≈ 247, 248, 244).
const PAPER_FLOOR: u8 = 238;
/// Canvas pixels off the lines, on a fine line and on a coarse one (the ground's grid).
const OFF_LINE: (u32, u32) = (10, 10);
const FINE_LINE: (u32, u32) = (20, 10);
const COARSE_LINE: (u32, u32) = (100, 10);
/// A wash of ultramarine over the whole sheet: its pigment share, and a thin and a thick coat.
const WASH_SHARE: f32 = 0.5;
const THIN_UM: f32 = 2.0;
const THICK_UM: f32 = 400.0;
/// How many 8-bit steps two pixels under the same paint may differ by (the present dithers).
const SAME_PAINT_STEPS: i32 = 2;
/// How many runs of `FRAMES` frames a stroke may take to settle.
const SETTLE_RUNS: u32 = 10;

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

fn rgb_at(pixels: &[u8], (x, y): (u32, u32)) -> [i32; 3] {
    let start = ((y * SIZE.0 + x) * 4) as usize;
    [0, 1, 2].map(|c| i32::from(pixels[start + c]))
}

/// How much darker the pixel at `line` shows than the one off the lines, summed over RGB.
fn line_darkness(pixels: &[u8], line: (u32, u32)) -> i32 {
    let (off, on) = (rgb_at(pixels, OFF_LINE), rgb_at(pixels, line));
    (0..3).map(|c| off[c] - on[c]).sum()
}

/// Covers the whole sheet with an even coat of ultramarine `volume_um` thick.
fn wash(gpu: &GpuContext, sheet: &mut TestSheet, volume_um: f32) {
    let texels = (SIZE.0 * SIZE.1) as usize;
    let canvas = &mut sheet.session.canvas;
    canvas.write_f32(gpu, StateTexture::WetVol, &vec![volume_um; texels]);
    let shares = [0.0, WASH_SHARE, 0.0, 0.0];
    canvas.write_f32(gpu, StateTexture::WetPig0, &shares.repeat(texels));
    canvas.dirty.mark_all();
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
    let paper = rgb_at(&blank, OFF_LINE);
    assert!(
        paper.iter().all(|c| *c >= i32::from(PAPER_FLOOR)),
        "{paper:?}"
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

/// Paints a row, then begins a second one with a bigger brush, ending the first stroke
/// first only when `ended` is set; the settled pixels.
fn second_stroke_after(gpu: &GpuContext, ended: bool) -> Vec<u8> {
    let mut sheet = round_sheet(gpu);
    let mut clock_ms = 0.0;
    let first = row(60.0);
    sheet.begin_stroke(gpu, first[0]);
    sheet.add_samples(&first[1..]);
    if ended {
        sheet.end_stroke();
    }
    sheet.set_brush(&BrushSpec {
        size_px: BRUSH_PX * 3.0,
        ..brush(HeadSpec::ROUND)
    });
    stroke(gpu, &mut sheet, &row(140.0));
    shown(gpu, &mut sheet, &mut clock_ms)
}

#[test]
fn a_stroke_left_open_ends_as_it_began_before_the_next_dip() {
    let gpu = GpuContext::new().unwrap();
    let left_open = second_stroke_after(&gpu, false);
    let ended = second_stroke_after(&gpu, true);
    let differing = left_open.iter().zip(&ended).filter(|(a, b)| a != b).count();
    assert_eq!(differing, 0, "{differing} bytes differ");
}

#[test]
fn clearing_mid_stroke_gives_back_a_blank_sheet() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = round_sheet(&gpu);
    let mut clock_ms = 0.0;
    let blank = shown(&gpu, &mut sheet, &mut clock_ms);
    let samples = row(60.0);
    sheet.begin_stroke(&gpu, samples[0]);
    sheet.add_samples(&samples[1..]);
    sheet.clear(&gpu).unwrap();
    let cleared = shown(&gpu, &mut sheet, &mut clock_ms);
    assert!(cleared == blank, "the cleared sheet is not blank");
    assert_eq!(wet_volume(&gpu, &sheet), 0.0);
}

#[test]
fn a_stroke_needs_frames_until_it_settles() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = round_sheet(&gpu);
    let mut clock_ms = 0.0;
    stroke(&gpu, &mut sheet, &row(60.0));
    assert!(sheet.needs_frames(clock_ms));
    let settled = (0..SETTLE_RUNS).any(|_| {
        shown(&gpu, &mut sheet, &mut clock_ms);
        !sheet.needs_frames(clock_ms)
    });
    assert!(settled, "still needs frames at {clock_ms} ms");
}

#[test]
fn a_blank_sheet_is_ruled_with_fine_and_coarse_lines() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = round_sheet(&gpu);
    let blank = shown(&gpu, &mut sheet, &mut 0.0);
    let (fine, coarse) = (
        line_darkness(&blank, FINE_LINE),
        line_darkness(&blank, COARSE_LINE),
    );
    assert!(fine > 0, "fine line {fine}");
    assert!(coarse > fine, "coarse line {coarse}, fine {fine}");
}

#[test]
fn clearing_keeps_the_ruled_lines() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = round_sheet(&gpu);
    let mut clock_ms = 0.0;
    wash(&gpu, &mut sheet, THICK_UM);
    shown(&gpu, &mut sheet, &mut clock_ms);
    sheet.clear(&gpu).unwrap();
    let cleared = shown(&gpu, &mut sheet, &mut clock_ms);
    assert!(line_darkness(&cleared, FINE_LINE) > 0);
    assert!(line_darkness(&cleared, COARSE_LINE) > line_darkness(&cleared, FINE_LINE));
}

#[test]
fn thin_paint_shows_the_lines_and_thick_paint_hides_them() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = round_sheet(&gpu);
    let mut clock_ms = 0.0;
    wash(&gpu, &mut sheet, THIN_UM);
    let thin = shown(&gpu, &mut sheet, &mut clock_ms);
    wash(&gpu, &mut sheet, THICK_UM);
    let thick = shown(&gpu, &mut sheet, &mut clock_ms);
    assert!(
        line_darkness(&thin, COARSE_LINE) > 3 * SAME_PAINT_STEPS,
        "{:?}",
        rgb_at(&thin, COARSE_LINE)
    );
    for line in [FINE_LINE, COARSE_LINE] {
        let (off, on) = (rgb_at(&thick, OFF_LINE), rgb_at(&thick, line));
        assert!(
            (0..3).all(|c| (off[c] - on[c]).abs() <= SAME_PAINT_STEPS),
            "{off:?} vs {on:?}"
        );
    }
}
