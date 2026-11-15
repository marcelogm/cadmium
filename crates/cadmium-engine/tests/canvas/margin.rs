use cadmium_engine::{
    canvas::{CanvasSpec, MARGIN_PX},
    gpu::GpuContext,
};

use crate::support::{
    frame, painting_request, painting_session, presented_rgba, without_wgpu_errors,
};

const GROUNDS_DIFFER_BY: u8 = 40;
const SAME_GROUND_WITHIN: u8 = 8;

fn furthest_channel(a: &[u8], b: &[u8]) -> u8 {
    a.iter()
        .zip(b)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or_default()
}

#[test]
fn the_margin_shows_a_ground_of_its_own_around_the_painting() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 256, 128);
    let (width, height) = (s.canvas.spec.width, s.canvas.spec.height);
    frame(&gpu, &mut s);
    let rgba = presented_rgba(&gpu, &s, width, height);
    let pixel = |x: u32, y: u32| &rgba[((y * width + x) * 4) as usize..][..3];
    let in_margin = pixel(MARGIN_PX / 2, MARGIN_PX / 2);
    let in_painting = pixel(width / 2, height / 2);
    assert!(furthest_channel(in_margin, in_painting) > GROUNDS_DIFFER_BY);
    let across = pixel(width - MARGIN_PX / 2, height - MARGIN_PX / 2);
    assert!(furthest_channel(across, in_margin) <= SAME_GROUND_WITHIN);
}

#[test]
fn a_session_swaps_one_margined_painting_for_another() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 256, 128);
    let next = CanvasSpec::for_painting(painting_request(192, 192));
    without_wgpu_errors(&gpu, || s.replace_document(&gpu, next.clone()).unwrap());
    assert_eq!(s.canvas.spec, next);
}
