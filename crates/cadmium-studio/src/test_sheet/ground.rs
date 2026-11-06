//! The test sheet's ground: plain paper ruled with a fine grid and a coarser one, in the
//! ground itself, so paint lies over the lines as ink over graph paper: thin paint lets them
//! show, opaque paint hides them.

use cadmium_engine::canvas::{Canvas, StateTexture, primer_reflectance};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::pigments::PigmentDb;
use cadmium_engine::spectral::BandValues;

/// Fine lines every this many canvas px, coarse ones every `COARSE_STEP_PX`: at the editor's
/// usual cover scale (≈ 0.81) they fall about 16 and 80 px apart on screen, like its paper.
const FINE_STEP_PX: u32 = 20;
const COARSE_STEP_PX: u32 = 5 * FINE_STEP_PX;
/// Two canvas px, so a line shown below 1:1 never falls between two samples and fades.
const LINE_PX: u32 = 2;
/// How much of the paper's reflectance a line keeps: at ≈ 0.81 on screen, about as dark
/// overall as the paper grid's one-pixel light and darker grey lines.
const FINE_SHARE: f32 = 0.9;
const COARSE_SHARE: f32 = 0.8;
/// Four of the eight bands in each of the two ground textures.
const BANDS_PER_TEXTURE: usize = 4;

/// How much of the paper canvas pixel (`x`, `y`) keeps: 1 off the lines; a coarse line wins
/// where it crosses a fine one.
pub(super) fn line_share(x: u32, y: u32) -> f32 {
    let on = |at: u32, step: u32| at % step < LINE_PX;
    if on(x, COARSE_STEP_PX) || on(y, COARSE_STEP_PX) {
        COARSE_SHARE
    } else if on(x, FINE_STEP_PX) || on(y, FINE_STEP_PX) {
        FINE_SHARE
    } else {
        1.0
    }
}

/// Rules the grid into `canvas`'s ground, the canvas's primer, which paint then lies over.
pub(super) fn rule(canvas: &mut Canvas, gpu: &GpuContext, db: &PigmentDb) {
    let paper = primer_reflectance(db, canvas.spec.primer);
    let size = (canvas.spec.width, canvas.spec.height);
    let textures = [
        (StateTexture::DryRefl0, 0),
        (StateTexture::DryRefl1, BANDS_PER_TEXTURE),
    ];
    for (which, first_band) in textures {
        canvas.write_f32(gpu, which, &ruled_texels(&paper, first_band, size));
    }
    canvas.dirty.mark_all();
}

/// One ground texture's texels, row by row: four bands of the paper, darkened on the lines.
fn ruled_texels(paper: &BandValues, first_band: usize, (width, height): (u32, u32)) -> Vec<f32> {
    let bands = &paper[first_band..first_band + BANDS_PER_TEXTURE];
    let mut texels = Vec::with_capacity((width * height) as usize * BANDS_PER_TEXTURE);
    for y in 0..height {
        for x in 0..width {
            let share = line_share(x, y);
            texels.extend(bands.iter().map(|band| band * share));
        }
    }
    texels
}

#[cfg(test)]
mod tests;
