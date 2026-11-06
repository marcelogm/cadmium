use cadmium_engine::brush::{BrushSpec, HeadSpec, factory_brushes};
use cadmium_engine::gpu::GpuContext;
use cadmium_studio::{BrushStudio, DemoStroke, THUMBNAIL_SIZE};

fn ink_share(rgba: &[u8]) -> f64 {
    let (pixels, _) = rgba.as_chunks::<4>();
    let inked = pixels
        .iter()
        .filter(|p| p[0] < 200 || p[1] < 200 || p[2] < 200)
        .count();
    inked as f64 / pixels.len() as f64
}

/// How far a channel may stray from the paper before a pixel counts as inked: well above
/// the smooth gesso's shading and sRGB rounding, well below even a thin glaze.
const INK_STEP: i32 = 24;

fn is_inked(pixel: &[u8; 4], paper: &[u8; 4]) -> bool {
    (0..3).any(|c| (i32::from(pixel[c]) - i32::from(paper[c])).abs() > INK_STEP)
}

/// The pixels that differ from the paper, read at the top-left corner, which the stroke
/// box's margins keep bare.
fn inked_pixels(rgba: &[u8]) -> Vec<[u8; 4]> {
    let (pixels, _) = rgba.as_chunks::<4>();
    let paper = pixels[0];
    pixels
        .iter()
        .filter(|p| is_inked(p, &paper))
        .copied()
        .collect()
}

/// The mean channel difference between two same-sized images over the pixels inked in
/// either, so the bare paper cannot dilute a shifted or smeared stroke.
fn ink_difference(a: &[u8], b: &[u8]) -> f64 {
    let ((a, _), (b, _)) = (a.as_chunks::<4>(), b.as_chunks::<4>());
    let (paper_a, paper_b) = (a[0], b[0]);
    let diffs: Vec<f64> = a
        .iter()
        .zip(b)
        .filter(|(pa, pb)| is_inked(pa, &paper_a) || is_inked(pb, &paper_b))
        .flat_map(|(pa, pb)| (0..3).map(|c| f64::from(pa[c].abs_diff(pb[c]))))
        .collect();
    diffs.iter().sum::<f64>() / diffs.len().max(1) as f64
}

fn mean_colour(pixels: &[[u8; 4]]) -> [f64; 3] {
    let sum = pixels.iter().fold([0.0; 3], |sum, p| {
        [0, 1, 2].map(|c| sum[c] + f64::from(p[c]))
    });
    sum.map(|channel| channel / pixels.len() as f64)
}

/// Averages each 2x2 block of a `width`-wide RGBA8 image.
fn halve(rgba: &[u8], width: usize) -> Vec<u8> {
    let height = rgba.len() / 4 / width;
    let at = |x: usize, y: usize, c: usize| u32::from(rgba[(y * width + x) * 4 + c]);
    let block = |x: usize, y: usize, c: usize| {
        let sum = at(x, y, c) + at(x + 1, y, c) + at(x, y + 1, c) + at(x + 1, y + 1, c);
        ((sum + 2) / 4) as u8
    };
    (0..height)
        .step_by(2)
        .flat_map(|y| (0..width).step_by(2).map(move |x| (x, y)))
        .flat_map(|(x, y)| (0..4).map(move |c| block(x, y, c)))
        .collect()
}

fn brush_with(head: HeadSpec) -> BrushSpec {
    factory_brushes()
        .into_iter()
        .find(|b| b.head == head)
        .unwrap()
}

/// About half of what each stroke inks with the first factory brush.
fn ink_floor(stroke: DemoStroke) -> f64 {
    match stroke {
        DemoStroke::SCurve => 0.12,
        DemoStroke::PressureRamp => 0.1,
    }
}

#[test]
fn a_thumbnail_has_the_thumbnail_size_and_some_paint() {
    let gpu = GpuContext::new().unwrap();
    let mut studio = BrushStudio::new(&gpu).unwrap();
    let thumb = studio.thumbnail(&gpu, &factory_brushes()[0]).unwrap();
    assert_eq!((thumb.width, thumb.height), THUMBNAIL_SIZE);
    assert_eq!(
        thumb.rgba.len(),
        (THUMBNAIL_SIZE.0 * THUMBNAIL_SIZE.1 * 4) as usize
    );
    let share = ink_share(&thumb.rgba);
    assert!(share > 0.02 && share < 0.8, "{share}");
}

#[test]
fn the_same_brush_twice_in_a_row_gives_the_same_thumbnail() {
    let gpu = GpuContext::new().unwrap();
    let mut studio = BrushStudio::new(&gpu).unwrap();
    let flat = brush_with(HeadSpec::FLAT);
    let first = studio.thumbnail(&gpu, &flat).unwrap();
    let again = studio.thumbnail(&gpu, &flat).unwrap();
    assert_eq!(first, again);
}

#[test]
fn another_brush_in_between_leaves_no_trace() {
    let gpu = GpuContext::new().unwrap();
    let mut studio = BrushStudio::new(&gpu).unwrap();
    let flat = brush_with(HeadSpec::FLAT);
    let first = studio.thumbnail(&gpu, &flat).unwrap();
    let mop = studio.thumbnail(&gpu, &brush_with(HeadSpec::MOP)).unwrap();
    let again = studio.thumbnail(&gpu, &flat).unwrap();
    assert_ne!(first, mop);
    assert_eq!(first, again);
}

#[test]
fn a_fine_brush_inks_less_than_a_broad_one() {
    let gpu = GpuContext::new().unwrap();
    let mut studio = BrushStudio::new(&gpu).unwrap();
    let rigger = brush_with(HeadSpec::RIGGER);
    let mop = brush_with(HeadSpec::MOP);
    let fine = ink_share(&studio.thumbnail(&gpu, &rigger).unwrap().rgba);
    let broad = ink_share(&studio.thumbnail(&gpu, &mop).unwrap().rgba);
    assert!(fine * 3.0 < broad, "rigger {fine} mop {broad}");
}

#[test]
fn every_demo_stroke_paints() {
    let gpu = GpuContext::new().unwrap();
    let mut studio = BrushStudio::new(&gpu).unwrap();
    for stroke in [DemoStroke::SCurve, DemoStroke::PressureRamp] {
        studio.run(&gpu, &factory_brushes()[0], stroke).unwrap();
        let share = ink_share(&studio.rgba(&gpu).unwrap());
        assert!(share > ink_floor(stroke), "{stroke:?} {share}");
    }
}

#[test]
fn thumbnails_are_rack_row_strokes_ten_by_three() {
    assert_eq!(THUMBNAIL_SIZE, (240, 72));
    assert_eq!(THUMBNAIL_SIZE.0 * 3, THUMBNAIL_SIZE.1 * 10);
}

#[test]
fn a_thumbnail_is_painted_in_ultramarine() {
    let gpu = GpuContext::new().unwrap();
    let mut studio = BrushStudio::new(&gpu).unwrap();
    let thumb = studio.thumbnail(&gpu, &factory_brushes()[0]).unwrap();
    let inked = inked_pixels(&thumb.rgba);
    assert!(!inked.is_empty());
    let [r, g, b] = mean_colour(&inked);
    assert!(
        b > r + 20.0 && b > g + 20.0,
        "mean inked colour {r} {g} {b}"
    );
}

#[test]
fn the_full_thumbnail_is_the_thumbnail_at_twice_the_size() {
    let gpu = GpuContext::new().unwrap();
    let mut studio = BrushStudio::new(&gpu).unwrap();
    let brush = brush_with(HeadSpec::FLAT);
    let thumb = studio.thumbnail(&gpu, &brush).unwrap();
    let full = studio.thumbnail_full(&gpu, &brush).unwrap();
    assert_eq!(
        (full.width, full.height),
        (THUMBNAIL_SIZE.0 * 2, THUMBNAIL_SIZE.1 * 2)
    );
    assert_eq!(full.rgba.len(), (full.width * full.height * 4) as usize);
    let halved = halve(&full.rgba, full.width as usize);
    let off = ink_difference(&halved, &thumb.rgba);
    // Aligned is about 1.9 on radv and lavapipe; a 1 px shift at full size reads 4.5.
    assert!(off < 3.0, "mean channel difference over the ink {off}");
    let (shown, kept) = (ink_share(&thumb.rgba), ink_share(&halved));
    assert!(
        (shown - kept).abs() < 0.1 * shown,
        "inked {shown} vs {kept}"
    );
}
