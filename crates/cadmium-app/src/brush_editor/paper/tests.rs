use cadmium_studio::{GROUND_COARSE_STEP_PX, GROUND_FINE_STEP_PX, GROUND_LINE_PX, TEST_SHEET_SIZE};

use gtk::cairo;

use super::{
    MAJOR_LINE, MAJOR_STEP, MINOR_LINE, MINOR_STEP, PAPER, draw_grid, ground_lines, paper_lines,
    rgb,
};
use crate::view::{cover_view, viewport_to_canvas};

/// The ground's lines every `step` canvas px across `extent` canvas px from `origin`: their
/// centres, in canvas px.
fn canvas_lines(origin: f64, extent: f64, step: u32) -> Vec<f64> {
    let centre = f64::from(GROUND_LINE_PX) / 2.0;
    (0..)
        .map(|k| f64::from(k * step) + centre)
        .skip_while(|&at| at < origin)
        .take_while(|&at| at < origin + extent)
        .collect()
}

/// A widget of `size` widget px at `scale` device px per widget px: the fallback's lines,
/// mapped onto the canvas through the cover the sheet is shown with, are the ground's.
fn assert_lines_match_the_ground(size: [f64; 2], scale: f64) {
    let viewport = (
        (size[0] * scale).ceil() as u32,
        (size[1] * scale).ceil() as u32,
    );
    let view = cover_view(TEST_SHEET_SIZE, viewport);
    for step in [GROUND_FINE_STEP_PX, GROUND_COARSE_STEP_PX] {
        let [across, down] = ground_lines(size, view, scale, step);
        let across: Vec<f64> = across
            .iter()
            .map(|&x| viewport_to_canvas(view, x * scale, 0.0).0)
            .collect();
        let down: Vec<f64> = down
            .iter()
            .map(|&y| viewport_to_canvas(view, 0.0, y * scale).1)
            .collect();
        let zoom = f64::from(view.zoom);
        let expected = [
            canvas_lines(f64::from(view.origin_x), size[0] * scale / zoom, step),
            canvas_lines(f64::from(view.origin_y), size[1] * scale / zoom, step),
        ];
        for (got, want) in [across, down].iter().zip(&expected) {
            assert!(!want.is_empty(), "{size:?} {scale} {step}");
            assert_close(got, want);
        }
    }
}

fn assert_close(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?} {want:?}");
    let close = got
        .iter()
        .zip(want)
        .all(|(got, want)| (got - want).abs() < 1e-3);
    assert!(close, "{got:?} {want:?}");
}

#[test]
fn a_tall_sheet_rules_its_first_frame_where_the_ground_is_ruled() {
    assert_lines_match_the_ground([400.0, 640.0], 1.0);
}

#[test]
fn a_wide_sheet_at_double_scale_rules_its_first_frame_where_the_ground_is_ruled() {
    assert_lines_match_the_ground([700.0, 300.0], 2.0);
}

#[test]
fn paper_lines_fall_on_pixel_centres_every_step() {
    let [across, down] = paper_lines([50.0, 33.0], 16.0);
    assert_eq!(across, vec![16.5, 32.5, 48.5]);
    assert_eq!(down, vec![16.5, 32.5]);
}

#[test]
fn paper_has_no_line_on_its_edges() {
    let [across, down] = paper_lines([32.0, 16.0], 16.0);
    assert_eq!(across, vec![16.5]);
    assert!(down.is_empty());
}

#[test]
fn colours_read_from_hex() {
    assert_eq!(rgb(0x010203), [1.0 / 255.0, 2.0 / 255.0, 3.0 / 255.0]);
}

#[test]
fn the_grid_paints_paper_with_lines_over_it() {
    let size = 100;
    let mut surface = cairo::ImageSurface::create(cairo::Format::Rgb24, size, size).unwrap();
    {
        let context = cairo::Context::new(&surface).unwrap();
        draw_grid(&context, [f64::from(size); 2]);
    }
    surface.flush();
    let stride = usize::try_from(surface.stride()).unwrap();
    let data = surface.data().unwrap();
    // Cairo stores RGB24 as little-endian 0x00RRGGBB: bytes B, G, R, x.
    let pixel = |x: usize, y: usize| {
        let at = y * stride + x * 4;
        [data[at + 2], data[at + 1], data[at]]
    };
    let byte = |colour: [f64; 3]| colour.map(|c| (c * 255.0).round() as u8);
    // Paper between lines, a minor line at its step, a major line over a minor one.
    let row = 8;
    assert_eq!(pixel(MINOR_STEP as usize / 2, row), byte(PAPER));
    assert_eq!(pixel(MINOR_STEP as usize, row), byte(MINOR_LINE));
    assert_eq!(pixel(MAJOR_STEP as usize, row), byte(MAJOR_LINE));
}
