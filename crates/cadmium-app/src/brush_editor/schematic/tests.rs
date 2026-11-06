use cadmium_engine::brush::{BrushSpec, HeadSpec, factory_brushes};
use gtk::cairo;

use super::canvas::{Bounds, Canvas, Mark, Part};
use super::pen::{GRID, PAPER};
use super::{Subject, paint};

/// The sheet's own size: drawn at one pixel per sheet unit.
const SIZE: [i32; 2] = [320, 582];
/// Pixels this close to the edge lie outside the sheet's border: only paper and grid there.
const MARGIN: usize = 4;
/// Antialiasing slack, in 8-bit levels, around the paper and grid colours.
const SLACK: f64 = 3.0;
const DARK: u8 = 100;
const MIN_INKED: usize = 500;

/// Draws `brush` on a sheet-sized image at `pixel_scale`, and gives the image with what landed
/// where.
fn drawn(brush: &BrushSpec, pixel_scale: f64) -> (cairo::ImageSurface, Vec<Mark>) {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, SIZE[0], SIZE[1])
        .expect("an image surface");
    let context = cairo::Context::new(&surface).expect("a cairo context");
    let canvas = Canvas::new(&context, pixel_scale);
    let subject = Subject {
        brush,
        head: None,
        hair: None,
    };
    paint(&canvas, &subject, SIZE);
    assert!(context.status().is_ok(), "{:?}", brush.head);
    let marks = canvas.marks.take();
    drop(canvas);
    drop(context);
    (surface, marks)
}

/// Each pixel's red, green and blue, row by row.
fn render(brush: &BrushSpec) -> Vec<[u8; 3]> {
    let (mut surface, _) = drawn(brush, 1.0);
    surface.flush();
    let stride = usize::try_from(surface.stride()).expect("a positive stride");
    let width = SIZE[0] as usize;
    let data = surface.data().expect("the surface's pixels");
    (0..width * SIZE[1] as usize)
        .map(|i| {
            let at = (i / width) * stride + (i % width) * 4;
            [data[at + 2], data[at + 1], data[at]]
        })
        .collect()
}

fn paper_or_grid(pixel: [u8; 3]) -> bool {
    (0..3).all(|channel| {
        let [low, high] = [
            PAPER[channel].min(GRID[channel]),
            PAPER[channel].max(GRID[channel]),
        ];
        let value = f64::from(pixel[channel]);
        value >= low * 255.0 - SLACK && value <= high * 255.0 + SLACK
    })
}

fn in_margin(index: usize) -> bool {
    let (width, height) = (SIZE[0] as usize, SIZE[1] as usize);
    let (x, y) = (index % width, index / width);
    x < MARGIN || y < MARGIN || x >= width - MARGIN || y >= height - MARGIN
}

fn assert_on_the_sheet(brush: &BrushSpec) {
    let pixels = render(brush);
    let stray = (0..pixels.len()).find(|&i| in_margin(i) && !paper_or_grid(pixels[i]));
    assert!(
        stray.is_none(),
        "ink at pixel {stray:?} for {:?}",
        brush.head
    );
}

/// A head with every control at one end of its range, the ends chosen by the bits of `corner`.
fn extreme_head(corner: u32) -> HeadSpec {
    let pick = |range: [f32; 2], bit: u32| range[((corner >> bit) & 1) as usize];
    HeadSpec {
        thickness: pick(HeadSpec::THICKNESS_RANGE, 0),
        length: pick(HeadSpec::LENGTH_RANGE, 1),
        dome: pick(HeadSpec::DOME_RANGE, 2),
        slant: pick(HeadSpec::SLANT_RANGE, 3),
        point: pick(HeadSpec::POINT_RANGE, 4),
        belly: pick(HeadSpec::BELLY_RANGE, 5),
        fan_deg: pick(HeadSpec::FAN_RANGE_DEG, 6),
        density_per_px: pick(HeadSpec::DENSITY_RANGE_PER_PX, 7),
    }
}

/// Every factory brush, then the first one with each extreme head.
fn every_case() -> impl Iterator<Item = BrushSpec> {
    let base = factory_brushes().swap_remove(0);
    let extremes = (0..256).map(move |corner| BrushSpec {
        head: extreme_head(corner),
        ..base.clone()
    });
    factory_brushes().into_iter().chain(extremes)
}

fn overlap(a: Bounds, b: Bounds) -> bool {
    (0..2).all(|axis| a[0][axis] < b[1][axis] && b[0][axis] < a[1][axis])
}

/// Whether two marks must stay apart: a figure or caption from anything else, but the fan's
/// angle may sit on the views (it is drawn on the front view, as in the mockup).
fn kept_apart(a: &Mark, b: &Mark) -> bool {
    let lettered =
        |mark: &Mark| matches!(mark.part, Part::Caption | Part::Dimension | Part::FanAngle);
    let on_view = |x: &Mark, y: &Mark| x.part == Part::FanAngle && y.part == Part::View;
    (lettered(a) || lettered(b)) && !on_view(a, b) && !on_view(b, a)
}

fn clashes(marks: &[Mark]) -> Vec<(Mark, Mark)> {
    let pairs = marks
        .iter()
        .enumerate()
        .flat_map(|(i, a)| marks[i + 1..].iter().map(move |b| (*a, *b)));
    pairs
        .filter(|(a, b)| kept_apart(a, b) && overlap(a.bounds, b.bounds))
        .collect()
}

#[test]
fn every_factory_brush_and_extreme_head_stays_inside_the_sheets_border() {
    for brush in every_case() {
        assert_on_the_sheet(&brush);
    }
}

#[test]
fn no_figure_or_caption_runs_into_a_view_the_title_block_or_another() {
    for brush in every_case() {
        let (_, marks) = drawn(&brush, 1.0);
        let clashes = clashes(&marks);
        assert!(clashes.is_empty(), "{:?}: {clashes:#?}", brush.head);
    }
}

#[test]
fn the_drawing_marks_every_part_it_lays_out() {
    let fan = factory_brushes().swap_remove(8);
    let (_, marks) = drawn(&fan, 1.0);
    let count = |part: Part| marks.iter().filter(|mark| mark.part == part).count();
    // Front, side and footprint; FRONT, SIDE and FOOTPRINT; L, W and t; the fan's angle.
    assert_eq!(count(Part::View), 3);
    assert_eq!(count(Part::Caption), 3);
    assert_eq!(count(Part::Dimension), 3);
    assert_eq!(count(Part::FanAngle), 1);
    assert_eq!(count(Part::TitleBlock), 1);
}

#[test]
fn the_drawing_puts_ink_on_the_paper() {
    let pixels = render(&factory_brushes().swap_remove(0));
    let inked = pixels
        .iter()
        .filter(|pixel| pixel.iter().all(|&c| c < DARK))
        .count();
    assert!(inked > MIN_INKED, "{inked}");
}

/// The sheet's frame at one device pixel per unit: snapped, its 0.8-wide line lies in one
/// pixel column (the sixth) instead of smearing over two.
#[test]
fn a_thin_straight_line_covers_one_device_pixel() {
    let (mut surface, _) = drawn(&factory_brushes().swap_remove(0), 1.0);
    surface.flush();
    let stride = usize::try_from(surface.stride()).expect("a positive stride");
    let row = 305 * stride;
    let data = surface.data().expect("the surface's pixels");
    let red = |x: usize| f64::from(data[row + x * 4 + 2]) / 255.0;
    let paper = PAPER[0];
    assert!(red(6) < paper - 0.3, "{}", red(6));
    assert!((red(5) - paper).abs() < 0.05 && (red(7) - paper).abs() < 0.05);
}
