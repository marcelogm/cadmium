use cadmium_engine::brush::{BrushSpec, HairSpec, HeadSpec, factory_brushes};
use gtk::cairo;

use super::canvas::{Bounds, Canvas, Mark, Part};
use super::pen::{GRID, PAPER};
use super::{Subject, paint};

/// The sheet's own size: drawn at one pixel per sheet unit.
const SIZE: [i32; 2] = [320, 582];
/// The frame's lines cover pixel `MARGIN` from each edge: pixel 6 on the left and top, pixel
/// `width - 6` (314) and `height - 6` (576) on the right and bottom. Pixels beyond them hold
/// only paper and grid, or nothing at all in the rounded-off corners.
const MARGIN: usize = 6;
/// The rounded corners' radius in pixels, at any fit: a libadwaita card's.
const CORNER: usize = 12;
/// Half the sheet's size: drawn at half a pixel per sheet unit.
const HALF_SIZE: [i32; 2] = [160, 291];
/// Antialiasing slack, in 8-bit levels, around the paper and grid colours.
const SLACK: f64 = 3.0;
const DARK: u8 = 100;
const MIN_INKED: usize = 500;

/// Draws `brush` on a sheet-sized image at `pixel_scale`, and gives the image with what landed
/// where.
fn drawn(brush: &BrushSpec, pixel_scale: f64) -> (cairo::ImageSurface, Vec<Mark>) {
    drawn_at(brush, SIZE, pixel_scale)
}

/// Draws `brush` fitted into an image of `size`, at `pixel_scale`.
fn drawn_at(
    brush: &BrushSpec,
    size: [i32; 2],
    pixel_scale: f64,
) -> (cairo::ImageSurface, Vec<Mark>) {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, size[0], size[1])
        .expect("an image surface");
    let context = cairo::Context::new(&surface).expect("a cairo context");
    let canvas = Canvas::new(&context, pixel_scale);
    let subject = Subject {
        brush,
        head: None,
        hair: None,
    };
    paint(&canvas, &subject, size);
    assert!(context.status().is_ok(), "{:?}", brush.head);
    let marks = canvas.marks.take();
    drop(canvas);
    drop(context);
    (surface, marks)
}

/// Each pixel's red, green, blue and alpha (premultiplied), row by row.
fn render(brush: &BrushSpec) -> Vec<[u8; 4]> {
    render_at(brush, SIZE)
}

/// Like `render`, fitted into an image of `size`.
fn render_at(brush: &BrushSpec, size: [i32; 2]) -> Vec<[u8; 4]> {
    let (mut surface, _) = drawn_at(brush, size, 1.0);
    surface.flush();
    let stride = usize::try_from(surface.stride()).expect("a positive stride");
    let width = size[0] as usize;
    let data = surface.data().expect("the surface's pixels");
    (0..width * size[1] as usize)
        .map(|i| {
            let at = (i / width) * stride + (i % width) * 4;
            [data[at + 2], data[at + 1], data[at], data[at + 3]]
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
    x < MARGIN || y < MARGIN || x > width - MARGIN || y > height - MARGIN
}

/// Whether the pixel lies in one of the sheet's corner squares, where the rounding is.
fn in_corner(index: usize) -> bool {
    let (width, height) = (SIZE[0] as usize, SIZE[1] as usize);
    let (x, y) = (index % width, index / width);
    (x < CORNER || x >= width - CORNER) && (y < CORNER || y >= height - CORNER)
}

/// Paper or grid, opaque; or, in a corner, rounded off (partly or wholly transparent).
fn blank(index: usize, [red, green, blue, alpha]: [u8; 4]) -> bool {
    if alpha < u8::MAX {
        return in_corner(index);
    }
    paper_or_grid([red, green, blue])
}

fn assert_on_the_sheet(brush: &BrushSpec) {
    let pixels = render(brush);
    let stray = (0..pixels.len()).find(|&i| in_margin(i) && !blank(i, pixels[i]));
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

/// How many random heads the layout checks draw, and the seed they are drawn from.
const RANDOM_HEADS: usize = 300;
const SEED: u64 = 0x5eed_c0ff_ee15_7a11;

/// A fixed sequence of numbers in [0, 1): xorshift64*, so every run draws the same heads.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let mixed = self.0.wrapping_mul(0x2545_f491_4f6c_dd1d);
        (mixed >> 40) as f32 / (1u64 << 24) as f32
    }

    fn within(&mut self, [low, high]: [f32; 2]) -> f32 {
        low + (high - low) * self.next()
    }
}

/// A head with every control somewhere in its range, and a hair with any wet point.
fn random_brush(draws: &mut Draws, base: &BrushSpec) -> BrushSpec {
    let head = HeadSpec {
        thickness: draws.within(HeadSpec::THICKNESS_RANGE),
        length: draws.within(HeadSpec::LENGTH_RANGE),
        dome: draws.within(HeadSpec::DOME_RANGE),
        slant: draws.within(HeadSpec::SLANT_RANGE),
        point: draws.within(HeadSpec::POINT_RANGE),
        belly: draws.within(HeadSpec::BELLY_RANGE),
        fan_deg: draws.within(HeadSpec::FAN_RANGE_DEG),
        density_per_px: draws.within(HeadSpec::DENSITY_RANGE_PER_PX),
    };
    let hair = HairSpec {
        wet_point: draws.within(HairSpec::WET_POINT_RANGE),
        ..base.hair
    };
    BrushSpec {
        head,
        hair,
        ..base.clone()
    }
}

/// Every factory brush, the first one with each extreme head, then a fixed sample of random
/// heads and wet points.
fn every_case() -> impl Iterator<Item = BrushSpec> {
    let base = factory_brushes().swap_remove(0);
    let extreme_base = base.clone();
    let extremes = (0..256).map(move |corner| BrushSpec {
        head: extreme_head(corner),
        ..extreme_base.clone()
    });
    let mut draws = Draws(SEED);
    let random = (0..RANDOM_HEADS).map(move |_| random_brush(&mut draws, &base));
    factory_brushes().into_iter().chain(extremes).chain(random)
}

fn overlap(a: Bounds, b: Bounds) -> bool {
    (0..2).all(|axis| a[0][axis] < b[1][axis] && b[0][axis] < a[1][axis])
}

/// Whether two marks must stay apart: a figure or caption from anything else, but the fan's
/// angle may sit on the front view (it is drawn on it).
fn kept_apart(a: &Mark, b: &Mark) -> bool {
    let lettered =
        |mark: &Mark| matches!(mark.part, Part::Caption | Part::Dimension | Part::FanAngle);
    let on_view = |x: &Mark, y: &Mark| x.part == Part::FanAngle && y.part == Part::FrontView;
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
    assert_eq!(count(Part::FrontView), 1);
    assert_eq!(count(Part::SideView), 1);
    assert_eq!(count(Part::Footprint), 1);
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
        .filter(|pixel| pixel[..3].iter().all(|&c| c < DARK))
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

/// Pixels along the top-left diagonal: one outside a 12-pixel rounding, one inside it (and
/// outside a 24-pixel one), whatever the fit.
const OUTSIDE_THE_ARC: usize = 2;
const INSIDE_THE_ARC: usize = 5;

#[test]
fn the_sheets_corners_are_rounded_like_a_card() {
    for size in [SIZE, HALF_SIZE] {
        let pixels = render_at(&factory_brushes().swap_remove(0), size);
        let width = size[0] as usize;
        let at = |x: usize, y: usize| pixels[y * width + x][3];
        let last = [width - 1, size[1] as usize - 1];
        for [x, y] in [[0, 0], [last[0], 0], [0, last[1]], last] {
            assert_eq!(at(x, y), 0, "corner pixel ({x}, {y}) at {size:?}");
        }
        assert_eq!(at(OUTSIDE_THE_ARC, OUTSIDE_THE_ARC), 0, "{size:?}");
        assert_eq!(at(INSIDE_THE_ARC, INSIDE_THE_ARC), u8::MAX, "{size:?}");
        assert_eq!(at(0, CORNER), u8::MAX, "{size:?}");
        assert_eq!(at(CORNER, 0), u8::MAX, "{size:?}");
    }
}
