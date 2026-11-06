use cadmium_engine::present::ViewTransform;

use super::{letterbox, paper_colour, view_for};
use crate::example::EXAMPLE_SHEET_SIZE;
use crate::studio::Thumbnail;

const SHADER_GREY: [u8; 4] = [192, 191, 188, 255];
const PAPER: [u8; 4] = [250, 249, 245, 255];

fn grey_sheet() -> Thumbnail {
    let (width, height) = EXAMPLE_SHEET_SIZE;
    Thumbnail {
        width,
        height,
        rgba: SHADER_GREY.repeat((width * height) as usize),
    }
}

fn at(sheet: &Thumbnail, x: usize, y: usize) -> [u8; 4] {
    let start = (y * sheet.width as usize + x) * 4;
    sheet.rgba[start..start + 4].try_into().unwrap()
}

fn set(sheet: &mut Thumbnail, x: usize, y: usize, pixel: [u8; 4]) {
    let start = (y * sheet.width as usize + x) * 4;
    sheet.rgba[start..start + 4].copy_from_slice(&pixel);
}

#[test]
fn a_canvas_of_the_sheets_size_is_shown_one_to_one() {
    let view = view_for(EXAMPLE_SHEET_SIZE);
    let want = ViewTransform {
        origin_x: 0.0,
        origin_y: 0.0,
        zoom: 1.0,
        rotation: 0.0,
    };
    assert_eq!(view, want);
}

#[test]
fn a_canvas_twice_the_sheet_is_shown_at_half() {
    let view = view_for((1820, 880));
    assert_eq!((view.origin_x, view.origin_y, view.zoom), (0.0, 0.0, 0.5));
}

#[test]
fn a_narrower_canvas_is_centred_between_equal_bands() {
    let view = view_for((450, 440));
    assert_eq!(
        (view.origin_x, view.origin_y, view.zoom),
        (-230.0, 0.0, 1.0)
    );
}

#[test]
fn only_the_bands_take_the_paper_colour() {
    let canvas = (450, 440);
    let mut sheet = grey_sheet();
    letterbox(&mut sheet, view_for(canvas), canvas, PAPER);
    assert_eq!(at(&sheet, 0, 0), PAPER);
    assert_eq!(at(&sheet, 229, 100), PAPER);
    assert_eq!(at(&sheet, 230, 100), SHADER_GREY);
    assert_eq!(at(&sheet, 679, 439), SHADER_GREY);
    assert_eq!(at(&sheet, 680, 0), PAPER);
    assert_eq!(at(&sheet, 909, 439), PAPER);
}

#[test]
fn a_canvas_that_fills_the_sheet_keeps_every_pixel() {
    let mut sheet = grey_sheet();
    letterbox(
        &mut sheet,
        view_for(EXAMPLE_SHEET_SIZE),
        EXAMPLE_SHEET_SIZE,
        PAPER,
    );
    assert_eq!(sheet, grey_sheet());
}

/// The bare corner's left half in one grey, its right half in a warmer one.
fn two_tone(x: usize) -> [u8; 4] {
    if x < 5 {
        [200, 200, 200, 255]
    } else {
        [220, 210, 200, 255]
    }
}

#[test]
fn the_paper_colour_is_the_mean_of_the_bare_corner() {
    let view = view_for(EXAMPLE_SHEET_SIZE);
    let mut sheet = grey_sheet();
    for y in 0..10 {
        for x in 0..10 {
            set(&mut sheet, x, y, two_tone(x));
        }
    }
    set(&mut sheet, 10, 0, [0, 0, 0, 255]);
    set(&mut sheet, 0, 10, [0, 0, 0, 255]);
    assert_eq!(paper_colour(&sheet, view, 10.0), [210, 205, 200, 255]);
}

#[test]
fn no_bare_corner_reads_as_white() {
    let view = view_for(EXAMPLE_SHEET_SIZE);
    assert_eq!(paper_colour(&grey_sheet(), view, 0.0), [255, 255, 255, 255]);
}
