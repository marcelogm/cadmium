//! How the factory brushes look: head and hair names and the hair tint.

use cadmium_engine::brush::{HairSpec, HeadSpec};

const HEADS: [(HeadSpec, &str); 9] = [
    (HeadSpec::ROUND, "Round"),
    (HeadSpec::FLAT, "Flat"),
    (HeadSpec::FAN, "Fan"),
    (HeadSpec::BRIGHT, "Bright"),
    (HeadSpec::FILBERT, "Filbert"),
    (HeadSpec::RIGGER, "Rigger"),
    (HeadSpec::ANGULAR, "Angular"),
    (HeadSpec::DAGGER, "Dagger"),
    (HeadSpec::MOP, "Mop"),
];

/// Each factory hair with its name and its linear-light bristle tint.
const FACTORY_HAIRS: [(HairSpec, &str, [f32; 3]); 5] = [
    (HairSpec::SYNTHETIC, "Synthetic", [0.55, 0.38, 0.12]),
    (HairSpec::HOG, "Hog Bristle", [0.80, 0.72, 0.55]),
    (HairSpec::SABLE, "Kolinsky Sable", [0.35, 0.12, 0.05]),
    (HairSpec::MONGOOSE, "Mongoose", [0.30, 0.25, 0.20]),
    (HairSpec::SQUIRREL, "Squirrel", [0.10, 0.07, 0.05]),
];

/// The factory name of a head, or `None` once any parameter was changed.
pub fn head_name(head: &HeadSpec) -> Option<&'static str> {
    HEADS.iter().find(|(h, _)| h == head).map(|&(_, n)| n)
}

/// The factory name of a hair, or `None` once any parameter was changed.
pub fn hair_name(hair: &HairSpec) -> Option<&'static str> {
    FACTORY_HAIRS
        .iter()
        .find(|(h, ..)| h == hair)
        .map(|&(_, name, _)| name)
}

/// Linear-light bristle colour for the factory hair nearest in stiffness; a
/// tie goes to the earlier one in `FACTORY_HAIRS` (synthetic, hog, sable,
/// mongoose, squirrel).
pub(crate) fn hair_tint(hair: &HairSpec) -> [f32; 3] {
    let distance = |h: &HairSpec| (h.stiffness - hair.stiffness).abs();
    FACTORY_HAIRS
        .iter()
        .min_by(|a, b| distance(&a.0).total_cmp(&distance(&b.0)))
        .map(|&(_, _, tint)| tint)
        .expect("the factory has hairs")
}

#[cfg(test)]
mod tests;
