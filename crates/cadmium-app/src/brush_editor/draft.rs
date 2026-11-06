//! The brush being edited: what it was, what it is now, and how it is saved.

use cadmium_engine::brush::{BrushSpec, HairSpec, HeadSpec, default_brush, factory_brushes};
use cadmium_studio::{HairPreset, HeadPreset};

use super::closing::Closing;
use super::controls::{Control, Part};

const COPY_SUFFIX: &str = " (copy)";
/// What a brush made from scratch is called until it is named; Save numbers it if taken.
const NEW_BRUSH_NAME: &str = "New Brush";

/// Where the edited brush came from: it decides how Save stores it and whether it can be deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Origin {
    /// A user brush, at this library index: saved over itself, deletable.
    User(usize),
    /// A factory brush, or a worn brush in no library ("Current Brush"): saving makes a new
    /// user brush.
    Factory,
    /// A brush made from scratch: saving adds it as a new user brush; until then there is
    /// nothing to copy or delete.
    New,
}

/// What Save does with the draft.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum SavePlan {
    /// Stores `Draft::saved_brush` over the user brush at this index.
    Replace(usize),
    /// Adds this brush as a new user brush.
    AddNew(BrushSpec),
}

#[derive(Clone, Debug)]
pub(super) struct Draft {
    original: BrushSpec,
    current: BrushSpec,
    origin: Origin,
    /// The preset each part follows; `None` is Custom, even when the values equal a preset's.
    head_preset: Option<HeadPreset>,
    hair_preset: Option<HairPreset>,
}

impl Draft {
    /// A draft of `original` with every value in range. Library brushes already are (files and
    /// imports are clamped as they are read); a worn brush in no library may be at a size the
    /// size row can't show, and would otherwise turn dirty when the row snaps it into range.
    /// Each part opens on the preset it is exactly, else on Custom.
    pub(super) fn open(original: BrushSpec, origin: Origin) -> Draft {
        let original = original.clamped();
        Draft {
            head_preset: HeadPreset::of(&original.head),
            hair_preset: HairPreset::of(&original.hair),
            current: original.clone(),
            original,
            origin,
        }
    }

    /// A brush made from scratch: a Round head with Synthetic hair at the Round brush's
    /// suggested size, called "New Brush", with both parts on Custom so every slider moves at
    /// once. Untouched, it is not dirty.
    pub(super) fn new_brush() -> Draft {
        let round = factory_brushes()
            .into_iter()
            .find(|brush| brush.head == HeadSpec::ROUND && brush.hair == HairSpec::SYNTHETIC)
            .unwrap_or_else(default_brush);
        let original = BrushSpec {
            name: NEW_BRUSH_NAME.to_owned(),
            ..round
        };
        Draft {
            current: original.clone(),
            original,
            origin: Origin::New,
            head_preset: None,
            hair_preset: None,
        }
    }

    pub(super) fn original(&self) -> &BrushSpec {
        &self.original
    }

    pub(super) fn current(&self) -> &BrushSpec {
        &self.current
    }

    /// Whether saving would store something else than the original: spaces around the name
    /// don't count, since the saved name is trimmed.
    pub(super) fn is_dirty(&self) -> bool {
        self.saved_brush() != self.original
    }

    /// Whether ending the editor this way must ask before dropping the changes.
    pub(super) fn asks_before(&self, closing: Closing) -> bool {
        closing == Closing::Cancel && self.is_dirty()
    }

    pub(super) fn set_name(&mut self, name: &str) {
        self.current.name = name.to_owned();
    }

    pub(super) fn set_size(&mut self, size_px: f32) {
        let [low, high] = BrushSpec::SIZE_RANGE_PX;
        self.current.size_px = size_px.clamp(low, high);
    }

    pub(super) fn head_preset(&self) -> Option<HeadPreset> {
        self.head_preset
    }

    pub(super) fn hair_preset(&self) -> Option<HairPreset> {
        self.hair_preset
    }

    /// Whether `part` follows no preset, so its sliders move.
    pub(super) fn is_custom(&self, part: Part) -> bool {
        match part {
            Part::Head => self.head_preset.is_none(),
            Part::Hair => self.hair_preset.is_none(),
        }
    }

    /// A preset takes its values; Custom (`None`) keeps the current ones and unlocks them.
    pub(super) fn pick_head(&mut self, preset: Option<HeadPreset>) {
        self.head_preset = preset;
        if let Some(preset) = preset {
            self.current.head = preset.spec();
        }
    }

    /// A preset takes its values; Custom (`None`) keeps the current ones and unlocks them.
    pub(super) fn pick_hair(&mut self, preset: Option<HairPreset>) {
        self.hair_preset = preset;
        if let Some(preset) = preset {
            self.current.hair = preset.spec();
        }
    }

    /// Moves one control; refused (false) while its part follows a preset.
    pub(super) fn set(&mut self, control: Control, value: f32) -> bool {
        if !self.is_custom(control.part()) {
            return false;
        }
        control.set(&mut self.current, value);
        true
    }

    /// The name the brush is saved under: the typed name trimmed, or the original name when
    /// nothing but spaces was typed.
    pub(super) fn chosen_name(&self) -> String {
        match self.current.name.trim() {
            "" => self.original.name.clone(),
            name => name.to_owned(),
        }
    }

    /// Where Save puts the draft: a user brush is saved over itself; anything else becomes a
    /// new brush, a factory one called "<name> (copy)" when its name was left as it was.
    pub(super) fn save_plan(&self) -> SavePlan {
        let name = self.chosen_name();
        match self.origin {
            Origin::User(index) => SavePlan::Replace(index),
            Origin::Factory if name == self.original.name => {
                SavePlan::AddNew(self.named(copy_name(&name)))
            }
            Origin::Factory | Origin::New => SavePlan::AddNew(self.named(name)),
        }
    }

    /// The brush as it is saved or exported: the current values under the chosen name.
    pub(super) fn saved_brush(&self) -> BrushSpec {
        self.named(self.chosen_name())
    }

    /// What Save as Copy adds: the current values as "<chosen name> (copy)".
    pub(super) fn copy(&self) -> BrushSpec {
        self.named(copy_name(&self.chosen_name()))
    }

    /// The library index Delete removes: only a user brush has one.
    pub(super) fn deletable(&self) -> Option<usize> {
        match self.origin {
            Origin::User(index) => Some(index),
            Origin::Factory | Origin::New => None,
        }
    }

    /// Whether Save as Copy has a brush to copy: not until a new brush is first saved.
    pub(super) fn copyable(&self) -> bool {
        self.origin != Origin::New
    }

    fn named(&self, name: String) -> BrushSpec {
        BrushSpec {
            name,
            ..self.current.clone()
        }
    }
}

/// "<name> (copy)"; a name that already ends in " (copy)" keeps it once, so a copy of a copy
/// is numbered by the library rather than called "(copy) (copy)".
fn copy_name(name: &str) -> String {
    let base = name.strip_suffix(COPY_SUFFIX).unwrap_or(name);
    format!("{base}{COPY_SUFFIX}")
}

#[cfg(test)]
mod tests;
