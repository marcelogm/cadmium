//! The heads and hairs a painter picks from (the factory ones) and what each is called.

use cadmium_engine::brush::{HairSpec, HeadSpec};

/// A factory head, in the order the editor lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadPreset {
    Round,
    Flat,
    Bright,
    Filbert,
    Fan,
    Rigger,
    Angular,
    Dagger,
    Mop,
}

impl HeadPreset {
    /// Every head preset, in the editor's order.
    pub const ALL: [HeadPreset; 9] = [
        HeadPreset::Round,
        HeadPreset::Flat,
        HeadPreset::Bright,
        HeadPreset::Filbert,
        HeadPreset::Fan,
        HeadPreset::Rigger,
        HeadPreset::Angular,
        HeadPreset::Dagger,
        HeadPreset::Mop,
    ];

    /// The name the editor shows for this head.
    pub fn name(self) -> &'static str {
        match self {
            HeadPreset::Round => "Round",
            HeadPreset::Flat => "Flat",
            HeadPreset::Bright => "Bright",
            HeadPreset::Filbert => "Filbert",
            HeadPreset::Fan => "Fan",
            HeadPreset::Rigger => "Rigger",
            HeadPreset::Angular => "Angular",
            HeadPreset::Dagger => "Dagger",
            HeadPreset::Mop => "Mop",
        }
    }

    /// The factory head this preset stands for.
    pub fn spec(self) -> HeadSpec {
        match self {
            HeadPreset::Round => HeadSpec::ROUND,
            HeadPreset::Flat => HeadSpec::FLAT,
            HeadPreset::Bright => HeadSpec::BRIGHT,
            HeadPreset::Filbert => HeadSpec::FILBERT,
            HeadPreset::Fan => HeadSpec::FAN,
            HeadPreset::Rigger => HeadSpec::RIGGER,
            HeadPreset::Angular => HeadSpec::ANGULAR,
            HeadPreset::Dagger => HeadSpec::DAGGER,
            HeadPreset::Mop => HeadSpec::MOP,
        }
    }

    /// The preset `head` is exactly, if any.
    pub fn of(head: &HeadSpec) -> Option<HeadPreset> {
        HeadPreset::ALL
            .into_iter()
            .find(|preset| preset.spec() == *head)
    }
}

/// A factory hair, in the order the editor lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HairPreset {
    Synthetic,
    Hog,
    Sable,
    Mongoose,
    Squirrel,
}

impl HairPreset {
    /// Every hair preset, in the editor's order.
    pub const ALL: [HairPreset; 5] = [
        HairPreset::Synthetic,
        HairPreset::Hog,
        HairPreset::Sable,
        HairPreset::Mongoose,
        HairPreset::Squirrel,
    ];

    /// The name the editor shows for this hair.
    pub fn name(self) -> &'static str {
        match self {
            HairPreset::Synthetic => "Synthetic",
            HairPreset::Hog => "Hog Bristle",
            HairPreset::Sable => "Kolinsky Sable",
            HairPreset::Mongoose => "Mongoose",
            HairPreset::Squirrel => "Squirrel",
        }
    }

    /// The factory hair this preset stands for.
    pub fn spec(self) -> HairSpec {
        match self {
            HairPreset::Synthetic => HairSpec::SYNTHETIC,
            HairPreset::Hog => HairSpec::HOG,
            HairPreset::Sable => HairSpec::SABLE,
            HairPreset::Mongoose => HairSpec::MONGOOSE,
            HairPreset::Squirrel => HairSpec::SQUIRREL,
        }
    }

    /// The preset `hair` is exactly, if any.
    pub fn of(hair: &HairSpec) -> Option<HairPreset> {
        HairPreset::ALL
            .into_iter()
            .find(|preset| preset.spec() == *hair)
    }
}

/// The factory name of a head, or `None` once any parameter was changed.
pub fn head_name(head: &HeadSpec) -> Option<&'static str> {
    HeadPreset::of(head).map(HeadPreset::name)
}

/// The factory name of a hair, or `None` once any parameter was changed.
pub fn hair_name(hair: &HairSpec) -> Option<&'static str> {
    HairPreset::of(hair).map(HairPreset::name)
}

#[cfg(test)]
mod tests;
