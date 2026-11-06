use cadmium_engine::brush::{HeadSpec, factory_brushes};
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
fn the_same_brush_gives_the_same_thumbnail_every_time() {
    let gpu = GpuContext::new().unwrap();
    let mut studio = BrushStudio::new(&gpu).unwrap();
    let brushes = factory_brushes();
    let first = studio.thumbnail(&gpu, &brushes[3]).unwrap();
    studio.thumbnail(&gpu, &brushes[14]).unwrap();
    let again = studio.thumbnail(&gpu, &brushes[3]).unwrap();
    assert_eq!(first.rgba, again.rgba);
}

#[test]
fn a_fine_brush_inks_less_than_a_broad_one() {
    let gpu = GpuContext::new().unwrap();
    let mut studio = BrushStudio::new(&gpu).unwrap();
    let brushes = factory_brushes();
    let rigger = brushes.iter().find(|b| b.head == HeadSpec::RIGGER).unwrap();
    let mop = brushes.iter().find(|b| b.head == HeadSpec::MOP).unwrap();
    let fine = ink_share(&studio.thumbnail(&gpu, rigger).unwrap().rgba);
    let broad = ink_share(&studio.thumbnail(&gpu, mop).unwrap().rgba);
    assert!(fine * 3.0 < broad, "rigger {fine} mop {broad}");
}

#[test]
fn every_demo_stroke_paints() {
    let gpu = GpuContext::new().unwrap();
    let mut studio = BrushStudio::new(&gpu).unwrap();
    for stroke in DemoStroke::ALL {
        studio.run(&gpu, &factory_brushes()[0], stroke).unwrap();
        let rgba = studio.rgba(&gpu).unwrap();
        assert!(ink_share(&rgba) > 0.001, "{stroke:?}");
    }
}
