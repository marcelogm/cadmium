use oleo_engine::canvas::{CanvasSpec, Primer, Weave};

const STARTING_SIZE: (u32, u32) = (2048, 1536);

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
}
