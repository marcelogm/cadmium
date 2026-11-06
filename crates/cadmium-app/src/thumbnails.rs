//! Brush thumbnails: what each brush's stroke looks like, cached by what shapes it.

use std::collections::HashMap;

use cadmium_engine::brush::{BrushSpec, HairSpec, HeadSpec};
use cadmium_studio::{BrushStudio, StudioError, Thumbnail};
use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::state::SharedState;

/// What a thumbnail depends on: the brush's head, hair and size, not its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ThumbnailKey {
    head: [u32; 8],
    hair: [u32; 12],
    size: u32,
}

impl ThumbnailKey {
    /// Destructures every field, so a new one must be keyed or ignored on purpose.
    pub fn of(brush: &BrushSpec) -> ThumbnailKey {
        let BrushSpec {
            name: _,
            size_px,
            head,
            hair,
        } = brush;
        ThumbnailKey {
            head: head_fields(head).map(f32::to_bits),
            hair: hair_fields(hair).map(f32::to_bits),
            size: size_px.to_bits(),
        }
    }
}

fn head_fields(head: &HeadSpec) -> [f32; 8] {
    let HeadSpec {
        thickness,
        length,
        dome,
        slant,
        point,
        belly,
        fan_deg,
        density_per_px,
    } = *head;
    [
        thickness,
        length,
        dome,
        slant,
        point,
        belly,
        fan_deg,
        density_per_px,
    ]
}

fn hair_fields(hair: &HairSpec) -> [f32; 12] {
    let HairSpec {
        stiffness,
        damping,
        static_friction,
        kinetic_friction,
        drag_anisotropy,
        bend_memory,
        clumping,
        wet_point,
        point_release,
        point_film,
        load_limit,
        wicking,
    } = *hair;
    [
        stiffness,
        damping,
        static_friction,
        kinetic_friction,
        drag_anisotropy,
        bend_memory,
        clumping,
        wet_point,
        point_release,
        point_film,
        load_limit,
        wicking,
    ]
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
    BrushStudio::new(&gpu)
}

/// The rack positions showing `key`, given the keys in rack order: every twin gets it.
pub fn rack_indices(shown: &[ThumbnailKey], key: ThumbnailKey) -> impl Iterator<Item = usize> {
    shown
        .iter()
        .enumerate()
        .filter(move |(_, shown)| **shown == key)
        .map(|(index, _)| index)
}

/// Wraps a studio thumbnail (straight sRGB RGBA8, unpadded rows) as a GDK texture.
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
