//! Trays saved before the glass palette rest on a lighter grey; their bare texels move onto
//! today's ground as they open, so an old tray matches a new one.

use crate::canvas::{StateTexture, f32_to_half};

/// The flat reflectance the tray's ground had before the glass palette.
const OLD_TRAY_REFLECTANCE: f32 = 0.75;
const BYTES_PER_TEXEL: usize = 8;

/// Rewrites the texels of a saved tray's dry reflectance (`raw`, `which`) that show the old
/// ground in every band to `ground`; dried paint and the other states are left alone.
pub(super) fn rebase_tray_ground(which: StateTexture, raw: &mut [u8], ground: f32) {
    if !matches!(which, StateTexture::DryRefl0 | StateTexture::DryRefl1) {
        return;
    }
    let (old, new) = (flat_texel(OLD_TRAY_REFLECTANCE), flat_texel(ground));
    for texel in raw.as_chunks_mut::<BYTES_PER_TEXEL>().0 {
        if *texel == old {
            *texel = new;
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
