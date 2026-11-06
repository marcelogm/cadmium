//! Brush thumbnails: what each brush's stroke looks like, cached by what shapes it.

use std::collections::HashMap;

use cadmium_engine::brush::BrushSpec;
use cadmium_studio::{BrushStudio, StudioError, Thumbnail};
use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::state::SharedState;

const HEAD_FIELDS: usize = 8;
const HAIR_FIELDS: usize = 12;
const KEY_FIELDS: usize = HEAD_FIELDS + HAIR_FIELDS + 1;

/// What a thumbnail depends on: the brush's head, hair and size, not its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ThumbnailKey([u32; KEY_FIELDS]);

impl ThumbnailKey {
    pub fn of(brush: &BrushSpec) -> ThumbnailKey {
        let head = &brush.head;
        let hair = &brush.hair;
        let fields = [
            head.thickness,
            head.length,
            head.dome,
            head.slant,
            head.point,
            head.belly,
            head.fan_deg,
            head.density_per_px,
            hair.stiffness,
            hair.damping,
            hair.static_friction,
            hair.kinetic_friction,
            hair.drag_anisotropy,
            hair.bend_memory,
            hair.clumping,
            hair.wet_point,
            hair.point_release,
            hair.point_film,
            hair.load_limit,
            hair.wicking,
            brush.size_px,
        ];
        ThumbnailKey(fields.map(f32::to_bits))
    }
}

/// Thumbnails rendered so far; `T` is a texture in the app and plain bytes in tests.
pub struct ThumbnailCache<T = gdk::Texture> {
    thumbnails: HashMap<ThumbnailKey, T>,
}

impl<T> Default for ThumbnailCache<T> {
    fn default() -> Self {
        ThumbnailCache {
            thumbnails: HashMap::new(),
        }
    }
}

impl<T> ThumbnailCache<T> {
    pub fn get(&self, key: &ThumbnailKey) -> Option<&T> {
        self.thumbnails.get(key)
    }

    pub fn insert(&mut self, key: ThumbnailKey, thumbnail: T) {
        self.thumbnails.insert(key, thumbnail);
    }

    /// Positions of the brushes with no thumbnail yet; twins of an earlier brush are left out.
    pub fn missing<'a>(&self, brushes: impl IntoIterator<Item = &'a BrushSpec>) -> Vec<usize> {
        let mut seen = Vec::new();
        brushes
            .into_iter()
            .map(ThumbnailKey::of)
            .enumerate()
            .filter(|(_, key)| self.get(key).is_none())
            .filter(|(_, key)| {
                let first = !seen.contains(key);
                seen.push(*key);
                first
            })
            .map(|(index, _)| index)
            .collect()
    }
}

/// The first library brush `cache` has no thumbnail for, copied out of the state.
pub fn next_missing<T>(state: &SharedState, cache: &ThumbnailCache<T>) -> Option<BrushSpec> {
    let state = state.borrow();
    let index = *cache.missing(state.brushes.all()).first()?;
    state.brushes.get(index).cloned()
}

/// Paints `brush`'s thumbnail; the state is borrowed only to reach the GPU, not while painting.
pub fn render(
    state: &SharedState,
    studio: &mut BrushStudio,
    brush: &BrushSpec,
) -> Result<(ThumbnailKey, Thumbnail), StudioError> {
    let gpu = state.borrow().gpu.clone();
    let thumbnail = studio.thumbnail(&gpu, brush)?;
    Ok((ThumbnailKey::of(brush), thumbnail))
}

/// Opens a studio on the app's GPU, borrowing the state only to reach it.
pub fn open_studio(state: &SharedState) -> Result<BrushStudio, StudioError> {
    let gpu = state.borrow().gpu.clone();
    Ok(BrushStudio::new(&gpu)?)
}

pub fn texture(thumbnail: Thumbnail) -> gdk::Texture {
    let stride = thumbnail.width as usize * 4;
    gdk::MemoryTexture::new(
        thumbnail.width as i32,
        thumbnail.height as i32,
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from_owned(thumbnail.rgba),
        stride,
    )
    .upcast()
}

#[cfg(test)]
mod tests;
