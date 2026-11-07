use cadmium_engine::brush::{BrushSpec, HairSpec, HeadSpec, factory_brushes};
use cadmium_engine::canvas::{Primer, Weave};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::params::SimParams;

use super::fit::ExampleFit;
use super::framing::view_for;
use super::{EXAMPLE_SHEET_SIZE, ExampleRequest, ExampleSheet};
use crate::demo::StrokeBox;
use crate::paint::{DEMO_PIGMENT, StudioPaint};
use crate::studio::Thumbnail;

/// The heavy quarter of the ramp inks at least a sixth more than the light one. A light touch
/// lays an unbroken mark, so the middle and heavy quarters both near a full band.
const MIN_RAMP_GAIN_DIVISOR: usize = 6;

/// Small, so the canvas is small and the tests stay quick on lavapipe.
const TEST_SIZE_PX: f32 = 12.0;
/// How far a channel may stray from the paper before a pixel counts as inked (as in the
/// thumbnail tests).
const INK_STEP: i32 = 24;

/// A canvas area, in canvas pixels: `[left, right]`, `[top, bottom]`.
struct Region {
    x: [f64; 2],
    y: [f64; 2],
}

fn brush_with(head: HeadSpec) -> BrushSpec {
    let brush = factory_brushes()
        .into_iter()
        .find(|b| b.head == head)
        .unwrap();
    BrushSpec {
        size_px: TEST_SIZE_PX,
        ..brush
    }
}

fn paint_in(pigment: &str) -> StudioPaint {
    StudioPaint {
        slots: vec!["titanium_white".into(), pigment.into()],
        primer: Primer::WhiteGesso,
        weave: Weave::Smooth,
        recipe: vec![(1, 1.0)],
        load: 1.0,
        medium: 0.1,
        params: SimParams::default(),
    }
}

fn paint(gpu: &GpuContext, sheet: &mut ExampleSheet, brush: &BrushSpec) -> Thumbnail {
    let paint = paint_in(DEMO_PIGMENT);
    sheet
        .paint(
            gpu,
            ExampleRequest {
                brush,
                paint: &paint,
            },
        )
        .unwrap()
}

/// A stroke's box, with half a reach above and below: the band its ink can reach without
/// crossing into the other stroke's.
fn band(area: StrokeBox, fit: &ExampleFit) -> Region {
    let half = f64::from(fit.reach_px) / 2.0;
    Region {
        x: [area.x, area.x + area.width],
        y: [area.y - half, area.y + area.height + half],
    }
}

/// The `which`-th quarter of `region`, left to right.
fn quarter(region: &Region, which: u32) -> Region {
    let step = (region.x[1] - region.x[0]) / 4.0;
    let left = region.x[0] + step * f64::from(which);
    Region {
        x: [left, left + step],
        y: region.y,
    }
}

fn is_inked(pixel: &[u8], paper: &[u8]) -> bool {
    (0..3).any(|c| (i32::from(pixel[c]) - i32::from(paper[c])).abs() > INK_STEP)
}

fn pixel(sheet: &Thumbnail, [x, y]: [usize; 2]) -> &[u8] {
    let start = (y * sheet.width as usize + x) * 4;
    &sheet.rgba[start..start + 4]
}

/// The sheet pixel showing canvas point `[x, y]`, clamped to the sheet.
fn sheet_pixel(fit: &ExampleFit, [x, y]: [f64; 2]) -> [usize; 2] {
    let view = view_for(fit.canvas);
    let zoom = f64::from(view.zoom);
    let to = |at: f64, origin: f32, size: u32| {
        ((at - f64::from(origin)) * zoom).clamp(0.0, f64::from(size)) as usize
    };
    [
        to(x, view.origin_x, EXAMPLE_SHEET_SIZE.0),
        to(y, view.origin_y, EXAMPLE_SHEET_SIZE.1),
    ]
}

/// How many sheet pixels in `region` differ from the paper, read at the top-left corner.
fn inked(sheet: &Thumbnail, fit: &ExampleFit, region: &Region) -> usize {
    let paper = pixel(sheet, [0, 0]).to_vec();
    let [left, top] = sheet_pixel(fit, [region.x[0], region.y[0]]);
    let [right, bottom] = sheet_pixel(fit, [region.x[1], region.y[1]]);
    (top..bottom)
        .flat_map(|y| (left..right).map(move |x| [x, y]))
        .filter(|&at| is_inked(pixel(sheet, at), &paper))
        .count()
}

fn mean_ink(sheet: &Thumbnail) -> [f64; 3] {
    let paper = pixel(sheet, [0, 0]).to_vec();
    let (pixels, _) = sheet.rgba.as_chunks::<4>();
    let inked: Vec<&[u8; 4]> = pixels.iter().filter(|p| is_inked(*p, &paper)).collect();
    assert!(!inked.is_empty());
    let sum = inked.iter().fold([0.0; 3], |sum, p| {
        [0, 1, 2].map(|c| sum[c] + f64::from(p[c]))
    });
    sum.map(|channel| channel / inked.len() as f64)
}

#[test]
fn an_example_sheet_is_sheet_sized_and_both_strokes_paint() {
    let gpu = GpuContext::new().unwrap();
    let brush = brush_with(HeadSpec::ROUND);
    let sheet = paint(&gpu, &mut ExampleSheet::new(), &brush);
    assert_eq!((sheet.width, sheet.height), EXAMPLE_SHEET_SIZE);
    assert_eq!(sheet.rgba.len(), (sheet.width * sheet.height * 4) as usize);
    let fit = ExampleFit::of(&brush);
    assert!(inked(&sheet, &fit, &band(fit.s_curve_box(), &fit)) > 0);
    assert!(inked(&sheet, &fit, &band(fit.ramp_box(), &fit)) > 0);
}

#[test]
fn the_same_draft_paints_the_same_sheet_whatever_came_before() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = ExampleSheet::new();
    let round = brush_with(HeadSpec::ROUND);
    let first = paint(&gpu, &mut sheet, &round);
    let fan = BrushSpec {
        size_px: 20.0,
        ..brush_with(HeadSpec::FAN)
    };
    let between = paint(&gpu, &mut sheet, &fan);
    let again = paint(&gpu, &mut sheet, &round);
    assert_ne!(first, between);
    assert_eq!(first, again);
    assert_eq!(first, paint(&gpu, &mut ExampleSheet::new(), &round));
}

#[test]
fn a_reused_session_paints_the_same_sheet_again() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = ExampleSheet::new();
    let round = brush_with(HeadSpec::ROUND);
    assert_ne!(round.hair, HairSpec::SABLE);
    let sable = BrushSpec {
        hair: HairSpec::SABLE,
        ..round.clone()
    };
    let first = paint(&gpu, &mut sheet, &round);
    let built_for = sheet.built_for.clone();
    let between = paint(&gpu, &mut sheet, &sable);
    assert_eq!(
        sheet.built_for, built_for,
        "the hair alone keeps the canvas spec"
    );
    let again = paint(&gpu, &mut sheet, &round);
    assert_ne!(first, between);
    assert_eq!(first, again);
}

#[test]
fn changing_the_head_changes_the_pixels() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = ExampleSheet::new();
    let round = brush_with(HeadSpec::ROUND);
    let longer = BrushSpec {
        head: HeadSpec {
            length: 3.0,
            ..HeadSpec::ROUND
        },
        ..round.clone()
    };
    let shown = paint(&gpu, &mut sheet, &round);
    assert_ne!(shown, paint(&gpu, &mut sheet, &brush_with(HeadSpec::FLAT)));
    assert_ne!(shown, paint(&gpu, &mut sheet, &longer));
}

#[test]
fn the_ramp_inks_more_toward_its_heavy_end() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = ExampleSheet::new();
    for head in [HeadSpec::ROUND, HeadSpec::FLAT] {
        let brush = brush_with(head);
        let fit = ExampleFit::of(&brush);
        let shown = paint(&gpu, &mut sheet, &brush);
        let ramp = band(fit.ramp_box(), &fit);
        let ink: Vec<usize> = (0..4)
            .map(|which| inked(&shown, &fit, &quarter(&ramp, which)))
            .collect();
        let (light, heavy) = (ink[0], ink[3]);
        assert!(
            heavy > light + light / MIN_RAMP_GAIN_DIVISOR,
            "{head:?} {ink:?}"
        );
        assert!(
            ink[1..].iter().all(|&more| more > light),
            "{head:?} {ink:?}"
        );
    }
}

#[test]
fn painting_in_steps_paints_the_same_sheet() {
    let gpu = GpuContext::new().unwrap();
    let brush = brush_with(HeadSpec::FLAT);
    let paint_used = paint_in(DEMO_PIGMENT);
    let request = ExampleRequest {
        brush: &brush,
        paint: &paint_used,
    };
    let mut stepped = ExampleSheet::new();
    stepped.begin(&gpu, request).unwrap();
    assert!(stepped.is_running());
    let parts: Vec<Option<Thumbnail>> = (0..3).map(|_| stepped.step(&gpu).unwrap()).collect();
    assert!(parts[0].is_none() && parts[1].is_none());
    assert!(!stepped.is_running());
    let whole = ExampleSheet::new().paint(&gpu, request).unwrap();
    assert_eq!(parts[2].as_ref(), Some(&whole));
}

#[test]
fn a_sheet_with_nothing_begun_steps_to_nothing() {
    let gpu = GpuContext::new().unwrap();
    let mut sheet = ExampleSheet::new();
    assert_eq!(sheet.step(&gpu).unwrap(), None);
    assert!(!sheet.is_running());
}

#[test]
fn the_sheet_is_painted_in_the_paint_it_is_given() {
    let gpu = GpuContext::new().unwrap();
    let brush = brush_with(HeadSpec::FLAT);
    let mut sheet = ExampleSheet::new();
    for (pigment, reddish) in [("cadmium_red", true), (DEMO_PIGMENT, false)] {
        let paint = paint_in(pigment);
        let shown = sheet
            .paint(
                &gpu,
                ExampleRequest {
                    brush: &brush,
                    paint: &paint,
                },
            )
            .unwrap();
        let [r, _, b] = mean_ink(&shown);
        assert_eq!(r > b + 20.0, reddish, "{pigment}: {r} vs {b}");
        assert_eq!(b > r + 20.0, !reddish, "{pigment}: {r} vs {b}");
    }
}
