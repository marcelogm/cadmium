use cadmium_engine::spectral::BandValues;

use super::{BANDS_PER_TEXTURE, COARSE_SHARE, FINE_SHARE, line_share, ruled_texels};

/// A paper whose bands all differ, so a band out of place shows.
const PAPER: BandValues = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
const SIZE: (u32, u32) = (40, 30);

#[test]
fn a_pixel_off_the_lines_keeps_the_whole_paper() {
    for at in [(10, 10), (2, 2), (19, 37), (99, 63)] {
        assert_eq!(line_share(at.0, at.1), 1.0, "{at:?}");
    }
}

#[test]
fn fine_lines_fall_every_fine_step_two_pixels_wide() {
    for at in [(20, 10), (21, 10), (10, 40), (10, 41), (180, 57)] {
        assert_eq!(line_share(at.0, at.1), FINE_SHARE, "{at:?}");
    }
    assert_eq!(line_share(22, 10), 1.0);
}

#[test]
fn coarse_lines_fall_every_fifth_fine_step_and_win_where_they_cross() {
    for at in [
        (0, 10),
        (100, 10),
        (101, 37),
        (10, 200),
        (100, 20),
        (40, 300),
    ] {
        assert_eq!(line_share(at.0, at.1), COARSE_SHARE, "{at:?}");
    }
}

fn texel(texels: &[f32], (x, y): (u32, u32)) -> &[f32] {
    let start = (y * SIZE.0 + x) as usize * BANDS_PER_TEXTURE;
    &texels[start..start + BANDS_PER_TEXTURE]
}

#[test]
fn the_second_ground_texture_holds_the_upper_four_bands() {
    let texels = ruled_texels(&PAPER, BANDS_PER_TEXTURE, SIZE);
    assert_eq!(texels.len(), (SIZE.0 * SIZE.1) as usize * BANDS_PER_TEXTURE);
    assert_eq!(texel(&texels, (10, 10)), &PAPER[4..8]);
    let crossing: Vec<f32> = PAPER[4..8].iter().map(|b| b * COARSE_SHARE).collect();
    assert_eq!(texel(&texels, (0, 0)), crossing.as_slice());
}
