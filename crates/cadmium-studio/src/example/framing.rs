//! Where the canvas sits in the example sheet, and what fills the sheet round it.

use cadmium_engine::present::ViewTransform;

use super::EXAMPLE_SHEET_SIZE;
use crate::studio::Thumbnail;

const WHITE: [u8; 4] = [255; 4];

/// The whole canvas, as large as fits in the sheet, centred.
pub(super) fn view_for((width, height): (u32, u32)) -> ViewTransform {
    let (sheet_w, sheet_h) = (
        f64::from(EXAMPLE_SHEET_SIZE.0),
        f64::from(EXAMPLE_SHEET_SIZE.1),
    );
    let (canvas_w, canvas_h) = (f64::from(width), f64::from(height));
    let zoom = (sheet_w / canvas_w).min(sheet_h / canvas_h);
    ViewTransform {
        origin_x: (-(sheet_w / zoom - canvas_w) / 2.0) as f32,
        origin_y: (-(sheet_h / zoom - canvas_h) / 2.0) as f32,
        zoom: zoom as f32,
    }
}

/// Paints `paper` over every sheet pixel that shows no canvas.
pub(super) fn letterbox(
    sheet: &mut Thumbnail,
    view: ViewTransform,
    canvas: (u32, u32),
    paper: [u8; 4],
) {
    let width = sheet.width as usize;
    let (pixels, _) = sheet.rgba.as_chunks_mut::<4>();
    for (index, pixel) in pixels.iter_mut().enumerate() {
        if !shows_canvas(view, canvas, pixel_at(index, width)) {
            *pixel = paper;
        }
    }
}

/// The mean colour of the sheet pixels showing the canvas's top-left `bare_px` square,
/// which the margins keep free of paint; white when none does.
pub(super) fn paper_colour(sheet: &Thumbnail, view: ViewTransform, bare_px: f32) -> [u8; 4] {
    let width = sheet.width as usize;
    let (pixels, _) = sheet.rgba.as_chunks::<4>();
    let in_corner = |index: usize| {
        let [x, y] = canvas_at(view, pixel_at(index, width));
        (0.0..bare_px).contains(&x) && (0.0..bare_px).contains(&y)
    };
    let corner: Vec<[u8; 4]> = pixels
        .iter()
        .enumerate()
        .filter(|(index, _)| in_corner(*index))
        .map(|(_, pixel)| *pixel)
        .collect();
    mean(&corner).unwrap_or(WHITE)
}

fn pixel_at(index: usize, width: usize) -> [u32; 2] {
    [(index % width) as u32, (index / width) as u32]
}

/// The canvas point a sheet pixel shows: the present shader's mapping, in `f32` like it.
fn canvas_at(view: ViewTransform, [x, y]: [u32; 2]) -> [f32; 2] {
    [
        view.origin_x + (x as f32 + 0.5) / view.zoom,
        view.origin_y + (y as f32 + 0.5) / view.zoom,
    ]
}

fn shows_canvas(view: ViewTransform, (width, height): (u32, u32), pixel: [u32; 2]) -> bool {
    let [x, y] = canvas_at(view, pixel);
    (0.0..width as f32).contains(&x) && (0.0..height as f32).contains(&y)
}

fn mean(pixels: &[[u8; 4]]) -> Option<[u8; 4]> {
    let count = u32::try_from(pixels.len())
        .ok()
        .filter(|&count| count > 0)?;
    let sums = pixels.iter().fold([0u32; 4], |sums, pixel| {
        [0, 1, 2, 3].map(|c| sums[c] + u32::from(pixel[c]))
    });
    Some(sums.map(|sum| ((sum + count / 2) / count) as u8))
}

#[cfg(test)]
mod tests;
