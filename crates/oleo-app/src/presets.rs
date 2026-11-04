use oleo_engine::brush::DEFAULT_LOAD;
use oleo_engine::canvas::{CanvasSpec, Primer, Weave};

const STARTING_SIZE: (u32, u32) = (2048, 1536);
const STARTING_PIGMENT: &str = "ultramarine_blue";
const WHITES: [&str; 2] = ["titanium_white", "zinc_white"];
pub const STARTING_LOAD: f32 = DEFAULT_LOAD;

pub const PRIMARIES: &[&str] = &[
    "titanium_white",
    "cadmium_yellow",
    "cadmium_red",
    "quinacridone_magenta",
    "ultramarine_blue",
    "phthalo_blue",
    "burnt_umber",
    "ivory_black",
];
pub const ZORN: &[&str] = &[
    "titanium_white",
    "yellow_ochre",
    "cadmium_red",
    "ivory_black",
];
pub const EARTHS: &[&str] = &[
    "titanium_white",
    "yellow_ochre",
    "burnt_sienna",
    "burnt_umber",
    "ivory_black",
    "ultramarine_blue",
];

pub struct PigmentPreset {
    pub name: &'static str,
    pub label: &'static str,
    pub pigments: &'static [&'static str],
}

pub const PIGMENT_PRESETS: [PigmentPreset; 3] = [
    PigmentPreset {
        name: "zorn",
        label: "Paleta limitada (Zorn)",
        pigments: ZORN,
    },
    PigmentPreset {
        name: "primaries",
        label: "Primárias",
        pigments: PRIMARIES,
    },
    PigmentPreset {
        name: "earths",
        label: "Terras",
        pigments: EARTHS,
    },
];

pub fn starting_canvas_spec() -> CanvasSpec {
    CanvasSpec {
        width: STARTING_SIZE.0,
        height: STARTING_SIZE.1,
        primer: Primer::WhiteGesso,
        weave: Weave::Linen,
        pigment_slots: PRIMARIES.iter().map(|id| id.to_string()).collect(),
    }
}

pub fn starting_recipe(slots: &[String]) -> Vec<(usize, f32)> {
    let slot = slots
        .iter()
        .position(|id| id == STARTING_PIGMENT)
        .or_else(|| slots.iter().position(|id| !WHITES.contains(&id.as_str())))
        .unwrap_or_default();
    vec![(slot, 1.0)]
}

#[cfg(test)]
mod tests {
    use oleo_engine::pigments::{MAX_SLOTS, PigmentDb};

    use super::*;

    #[test]
    fn starting_painting_is_valid_and_fills_every_slot_with_the_primaries() {
        let spec = starting_canvas_spec();
        assert!(spec.validate(&PigmentDb::builtin()).is_ok());
        assert_eq!(spec.pigment_slots.len(), MAX_SLOTS);
        assert_eq!(spec.pigment_slots, PRIMARIES);
    }

    #[test]
    fn every_preset_is_a_valid_palette() {
        let db = PigmentDb::builtin();
        for preset in &PIGMENT_PRESETS {
            let spec = CanvasSpec {
                pigment_slots: preset.pigments.iter().map(|id| id.to_string()).collect(),
                ..starting_canvas_spec()
            };
            assert!(spec.validate(&db).is_ok(), "{}", preset.name);
        }
    }

    fn slots(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn first_brush_carries_ultramarine_when_the_palette_has_it() {
        let spec = starting_canvas_spec();
        let recipe = starting_recipe(&spec.pigment_slots);
        assert_eq!(spec.pigment_slots[recipe[0].0], "ultramarine_blue");
    }

    #[test]
    fn first_brush_skips_the_whites_when_there_is_no_ultramarine() {
        assert_eq!(starting_recipe(&slots(ZORN)), [(1, 1.0)]);
        assert_eq!(
            starting_recipe(&slots(&["zinc_white", "titanium_white", "ivory_black"])),
            [(2, 1.0)]
        );
    }

    #[test]
    fn an_all_white_palette_starts_with_the_first_white() {
        assert_eq!(starting_recipe(&slots(&["titanium_white"])), [(0, 1.0)]);
    }
}
