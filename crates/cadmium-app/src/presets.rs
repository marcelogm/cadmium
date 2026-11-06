use cadmium_engine::brush::DEFAULT_LOAD;
use cadmium_engine::canvas::{CanvasSpec, Primer, Weave};

const STARTING_SIZE: (u32, u32) = (2048, 1536);
const STARTING_PIGMENT: &str = "ultramarine_blue";
/// The whites, which show nothing on a white primer.
pub const WHITES: [&str; 2] = ["titanium_white", "zinc_white"];
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
        label: "Limited Palette (Zorn)",
        pigments: ZORN,
    },
    PigmentPreset {
        name: "primaries",
        label: "Primaries",
        pigments: PRIMARIES,
    },
    PigmentPreset {
        name: "earths",
        label: "Earths",
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
mod tests;
