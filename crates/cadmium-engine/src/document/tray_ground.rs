//! Trays saved before the glass palette rest on a lighter grey; their bare texels move onto
//! today's ground as they open, so an old tray matches a new one.

use crate::canvas::f32_to_half;

/// The flat reflectance the tray's ground had before the glass palette.
const OLD_TRAY_REFLECTANCE: f32 = 0.75;
const BYTES_PER_TEXEL: usize = 8;

/// Rewrites to `ground` the texels of a saved tray's dry reflectance (its lower bands in
/// `low`, its upper in `high`, laid out alike) that show the old ground in all eight bands;
/// dried paint keeps what it shows.
pub(super) fn rebase_tray_ground(low: &mut [u8], high: &mut [u8], ground: f32) {
    let (old, new) = (flat_texel(OLD_TRAY_REFLECTANCE), flat_texel(ground));
    let low = low.as_chunks_mut::<BYTES_PER_TEXEL>().0;
    let high = high.as_chunks_mut::<BYTES_PER_TEXEL>().0;
    for (low, high) in low.iter_mut().zip(high) {
        if *low == old && *high == old {
            (*low, *high) = (new, new);
        }
    }
}

/// The bytes of a texel holding `reflectance` in all four of its half-float bands.
fn flat_texel(reflectance: f32) -> [u8; BYTES_PER_TEXEL] {
    let [low, high] = f32_to_half(reflectance).to_le_bytes();
    [low, high, low, high, low, high, low, high]
}

#[cfg(test)]
mod tests;
