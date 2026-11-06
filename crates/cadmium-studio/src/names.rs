//! Factory names for head and hair specs, for the editor's summary.

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

const HAIRS: [(HairSpec, &str); 5] = [
    (HairSpec::SYNTHETIC, "Synthetic"),
    (HairSpec::HOG, "Hog Bristle"),
    (HairSpec::SABLE, "Kolinsky Sable"),
    (HairSpec::MONGOOSE, "Mongoose"),
    (HairSpec::SQUIRREL, "Squirrel"),
];

/// The factory name of a head, or `None` once any parameter was changed.
pub fn head_name(head: &HeadSpec) -> Option<&'static str> {
    HEADS.iter().find(|(h, _)| h == head).map(|&(_, n)| n)
}

/// The factory name of a hair, or `None` once any parameter was changed.
pub fn hair_name(hair: &HairSpec) -> Option<&'static str> {
    HAIRS.iter().find(|(h, _)| h == hair).map(|&(_, n)| n)
}

#[cfg(test)]
mod tests;
