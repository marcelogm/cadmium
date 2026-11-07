//! Brushes that ship with Cadmium: a head and a hair, at a suggested size.

use super::{HairSpec, HeadSpec};

#[derive(Clone, Debug, PartialEq)]
pub struct BrushSpec {
    pub name: String,
    pub size_px: f32,
    pub head: HeadSpec,
    pub hair: HairSpec,
}

impl BrushSpec {
    pub const SIZE_RANGE_PX: [f32; 2] = [1.0, 500.0];

    /// Same brush with every value in range.
    pub fn clamped(&self) -> BrushSpec {
        BrushSpec {
            name: self.name.clone(),
            size_px: super::within(self.size_px, Self::SIZE_RANGE_PX, 40.0),
            head: self.head.clamped(),
            hair: self.hair.clamped(),
        }
    }
}

const FACTORY: [(&str, f32, HeadSpec, HairSpec); 15] = [
    (
        "Round · Synthetic",
        40.0,
        HeadSpec::ROUND,
        HairSpec::SYNTHETIC,
    ),
    (
        "Round · Kolinsky Sable",
        24.0,
        HeadSpec::ROUND,
        HairSpec::SABLE,
    ),
    ("Round · Hog Bristle", 40.0, HeadSpec::ROUND, HairSpec::HOG),
    ("Flat · Hog Bristle", 56.0, HeadSpec::FLAT, HairSpec::HOG),
    (
        "Flat · Synthetic",
        48.0,
        HeadSpec::FLAT,
        HairSpec::SYNTHETIC,
    ),
    (
        "Bright · Hog Bristle",
        40.0,
        HeadSpec::BRIGHT,
        HairSpec::HOG,
    ),
    (
        "Filbert · Hog Bristle",
        48.0,
        HeadSpec::FILBERT,
        HairSpec::HOG,
    ),
    (
        "Filbert · Mongoose",
        40.0,
        HeadSpec::FILBERT,
        HairSpec::MONGOOSE,
    ),
    ("Fan · Hog Bristle", 64.0, HeadSpec::FAN, HairSpec::HOG),
    ("Fan · Synthetic", 56.0, HeadSpec::FAN, HairSpec::SYNTHETIC),
    (
        "Rigger · Synthetic",
        8.0,
        HeadSpec::RIGGER,
        HairSpec::SYNTHETIC,
    ),
    (
        "Rigger · Kolinsky Sable",
        10.0,
        HeadSpec::RIGGER,
        HairSpec::SABLE,
    ),
    (
        "Angular · Synthetic",
        40.0,
        HeadSpec::ANGULAR,
        HairSpec::SYNTHETIC,
    ),
    (
        "Dagger · Synthetic",
        32.0,
        HeadSpec::DAGGER,
        HairSpec::SYNTHETIC,
    ),
    ("Mop · Squirrel", 96.0, HeadSpec::MOP, HairSpec::SQUIRREL),
];

pub fn factory_brushes() -> Vec<BrushSpec> {
    FACTORY
        .iter()
        .map(|&(name, size_px, head, hair)| BrushSpec {
            name: name.to_string(),
            size_px,
            head,
            hair,
        })
        .collect()
}

/// The brush a new session holds: the first factory brush.
pub fn default_brush() -> BrushSpec {
    factory_brushes().swap_remove(0)
}

#[cfg(test)]
mod tests;
