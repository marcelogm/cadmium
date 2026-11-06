use cadmium_engine::brush::{HairSpec, HeadSpec, factory_brushes};
use cadmium_studio::TipGeometry;
use gtk::{cairo, gdk};

use super::{ICON_SIZE, LINE_WIDTH, PADDING, draw};

/// The pixels a line drawn on the padding's edge may reach: half its width, and the half
/// pixel the snapping may move it.
const RING: usize = (PADDING - LINE_WIDTH / 2.0 - 0.5) as usize;
const SIDE: usize = ICON_SIZE as usize;
/// A fitted drawing spans the room inside the padding along one axis, less a pixel.
const MIN_SPAN: usize = SIDE - 2 * PADDING as usize - 1;
const STRONG_INK: u8 = 128;
/// The fewest fully opaque pixels in a row that make the mouth line read as crisp.
const MIN_CRISP_RUN: usize = 6;

/// Each pixel's alpha, row by row.
fn alphas(head: &HeadSpec, hair: &HairSpec) -> Vec<u8> {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, ICON_SIZE, ICON_SIZE)
        .expect("an image surface");
    {
        let context = cairo::Context::new(&surface).expect("a cairo context");
        let ink = gdk::RGBA::new(1.0, 1.0, 1.0, 1.0);
        let view = TipGeometry::new(head, hair).front;
        draw(&context, &view, &ink, [ICON_SIZE; 2], 1.0);
    }
    surface.flush();
    let stride = usize::try_from(surface.stride()).expect("a positive stride");
    let data = surface.data().expect("the surface's pixels");
    (0..SIDE * SIDE)
        .map(|i| data[(i / SIDE) * stride + (i % SIDE) * 4 + 3])
        .collect()
}

#[test]
fn each_factory_tip_inks_inside_its_box() {
    for brush in factory_brushes() {
        let alphas = alphas(&brush.head, &brush.hair);
        let inked: Vec<(usize, usize)> = (0..SIDE * SIDE)
            .filter(|&i| alphas[i] > 0)
            .map(|i| (i % SIDE, i / SIDE))
            .collect();
        assert!(
            alphas.iter().any(|&alpha| alpha >= STRONG_INK),
            "{} draws no line",
            brush.name
        );
        let inside = |at: usize| (RING..SIDE - RING).contains(&at);
        assert!(
            inked.iter().all(|&(x, y)| inside(x) && inside(y)),
            "{} inks the outer {RING} px",
            brush.name
        );
        let span = |axis: fn(&(usize, usize)) -> usize| {
            let (low, high) = inked
                .iter()
                .map(axis)
                .fold((SIDE, 0), |(low, high), at| (low.min(at), high.max(at)));
            high + 1 - low
        };
        assert!(
            span(|&(x, _)| x).max(span(|&(_, y)| y)) >= MIN_SPAN,
            "{} does not fill its box",
            brush.name
        );
    }
}

/// The longest run of fully opaque pixels along any row.
fn longest_opaque_run(alphas: &[u8]) -> usize {
    alphas
        .chunks(SIDE)
        .flat_map(|row| row.split(|&alpha| alpha < u8::MAX))
        .map(<[u8]>::len)
        .max()
        .unwrap_or(0)
}

#[test]
fn each_factory_tip_draws_its_mouth_line_on_whole_pixels() {
    for brush in factory_brushes() {
        let run = longest_opaque_run(&alphas(&brush.head, &brush.hair));
        assert!(
            run >= MIN_CRISP_RUN,
            "{} has no crisp horizontal line (longest opaque run {run})",
            brush.name
        );
    }
}
