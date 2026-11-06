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
        DemoStroke::RisingLine => 0.1,
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
    for stroke in [DemoStroke::SCurve, DemoStroke::RisingLine] {
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
