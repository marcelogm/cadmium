use crate::spectral::{BANDS, BandValues, composite_over, km_layer, reflectance_srgb8};

pub const MAX_SLOTS: usize = 8;
pub const PROVISIONAL_PROVENANCE: &str = "Provisório: espectro ajustado à mão, não medido";
pub const MASSTONE_THICKNESS_UM: f32 = 1000.0;
pub const GLAZE_THICKNESS_UM: f32 = 30.0;
pub const GLAZE_PIGMENT_FRACTION: f32 = 0.3;
pub const TINT_WHITE_ID: &str = "titanium_white";
pub const BLACK_REFLECTANCE: BandValues = [0.0; BANDS];

const UNIFORM_SCATTERING_SHAPE: BandValues = [1.0; BANDS];
const WHITE_SCATTERING_SHAPE: BandValues = [1.3, 1.2, 1.1, 1.05, 1.0, 0.95, 0.9, 0.85];

#[derive(Clone, Debug, PartialEq)]
pub struct Pigment {
    pub id: &'static str,
    pub name: &'static str,
    pub colour_index: &'static str,
    pub provenance: &'static str,
    pub absorption: BandValues,
    pub scattering: BandValues,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mixture {
    pub volume_um: f32,
    pub fractions: [f32; MAX_SLOTS],
}

impl Mixture {
    pub fn is_empty(&self) -> bool {
        self.volume_um <= 0.0 || self.fractions.iter().all(|fraction| *fraction <= 0.0)
    }
}

#[derive(Clone, Debug)]
pub struct PigmentDb {
    pigments: Vec<Pigment>,
}

struct PigmentRecipe {
    id: &'static str,
    name: &'static str,
    colour_index: &'static str,
    masstone_reflectance: BandValues,
    scattering_power: f32,
    scattering_shape: BandValues,
}

const RECIPES: [PigmentRecipe; 12] = [
    PigmentRecipe {
        id: "titanium_white",
        name: "Branco de titânio",
        colour_index: "PW6",
        masstone_reflectance: [0.90, 0.92, 0.93, 0.93, 0.93, 0.93, 0.93, 0.93],
        scattering_power: 0.40,
        scattering_shape: WHITE_SCATTERING_SHAPE,
    },
    PigmentRecipe {
        id: "zinc_white",
        name: "Branco de zinco",
        colour_index: "PW4",
        masstone_reflectance: [0.85, 0.90, 0.91, 0.91, 0.91, 0.91, 0.91, 0.91],
        scattering_power: 0.12,
        scattering_shape: WHITE_SCATTERING_SHAPE,
    },
    PigmentRecipe {
        id: "cadmium_yellow",
        name: "Amarelo cádmio",
        colour_index: "PY35",
        masstone_reflectance: [0.05, 0.06, 0.25, 0.72, 0.84, 0.86, 0.87, 0.87],
        scattering_power: 0.15,
        scattering_shape: UNIFORM_SCATTERING_SHAPE,
    },
    PigmentRecipe {
        id: "yellow_ochre",
        name: "Ocre amarelo",
        colour_index: "PY43",
        masstone_reflectance: [0.06, 0.07, 0.10, 0.20, 0.35, 0.45, 0.50, 0.52],
        scattering_power: 0.12,
        scattering_shape: UNIFORM_SCATTERING_SHAPE,
    },
    PigmentRecipe {
        id: "cadmium_red",
        name: "Vermelho cádmio",
        colour_index: "PR108",
        masstone_reflectance: [0.05, 0.04, 0.04, 0.04, 0.06, 0.35, 0.65, 0.70],
        scattering_power: 0.15,
        scattering_shape: UNIFORM_SCATTERING_SHAPE,
    },
    PigmentRecipe {
        id: "quinacridone_magenta",
        name: "Quinacridona magenta",
        colour_index: "PR122",
        masstone_reflectance: [0.30, 0.20, 0.08, 0.04, 0.04, 0.15, 0.45, 0.60],
        scattering_power: 0.03,
        scattering_shape: UNIFORM_SCATTERING_SHAPE,
    },
    PigmentRecipe {
        id: "ultramarine_blue",
        name: "Azul ultramar",
        colour_index: "PB29",
        masstone_reflectance: [0.30, 0.40, 0.20, 0.07, 0.04, 0.03, 0.04, 0.08],
        scattering_power: 0.03,
        scattering_shape: UNIFORM_SCATTERING_SHAPE,
    },
    PigmentRecipe {
        id: "phthalo_blue",
        name: "Azul ftalo",
        colour_index: "PB15",
        masstone_reflectance: [0.12, 0.20, 0.15, 0.05, 0.02, 0.015, 0.015, 0.02],
        scattering_power: 0.02,
        scattering_shape: UNIFORM_SCATTERING_SHAPE,
    },
    PigmentRecipe {
        id: "phthalo_green",
        name: "Verde ftalo",
        colour_index: "PG7",
        masstone_reflectance: [0.04, 0.08, 0.18, 0.12, 0.05, 0.02, 0.015, 0.015],
        scattering_power: 0.02,
        scattering_shape: UNIFORM_SCATTERING_SHAPE,
    },
    PigmentRecipe {
        id: "burnt_sienna",
        name: "Siena queimada",
        colour_index: "PBr7",
        masstone_reflectance: [0.04, 0.04, 0.045, 0.05, 0.08, 0.18, 0.28, 0.32],
        scattering_power: 0.04,
        scattering_shape: UNIFORM_SCATTERING_SHAPE,
    },
    PigmentRecipe {
        id: "burnt_umber",
        name: "Sombra queimada",
        colour_index: "PBr7",
        masstone_reflectance: [0.03, 0.03, 0.035, 0.04, 0.05, 0.07, 0.09, 0.10],
        scattering_power: 0.08,
        scattering_shape: UNIFORM_SCATTERING_SHAPE,
    },
    PigmentRecipe {
        id: "ivory_black",
        name: "Preto marfim",
        colour_index: "PBk9",
        masstone_reflectance: [0.03; BANDS],
        scattering_power: 0.06,
        scattering_shape: UNIFORM_SCATTERING_SHAPE,
    },
];

impl PigmentRecipe {
    fn pigment(&self) -> Pigment {
        let scattering = self
            .scattering_shape
            .map(|shape| self.scattering_power * shape);
        let mut absorption = [0.0; BANDS];
        for band in 0..BANDS {
            let r = self.masstone_reflectance[band];
            absorption[band] = scattering[band] * (1.0 - r) * (1.0 - r) / (2.0 * r);
        }
        Pigment {
            id: self.id,
            name: self.name,
            colour_index: self.colour_index,
            provenance: PROVISIONAL_PROVENANCE,
            absorption,
            scattering,
        }
    }
}

impl PigmentDb {
    pub const VERSION: u32 = 1;

    pub fn builtin() -> PigmentDb {
        PigmentDb {
            pigments: RECIPES.iter().map(PigmentRecipe::pigment).collect(),
        }
    }

    pub fn get(&self, id: &str) -> Option<&Pigment> {
        self.pigments.iter().find(|pigment| pigment.id == id)
    }

    pub fn all(&self) -> &[Pigment] {
        &self.pigments
    }

    pub fn masstone_srgb8(&self, id: &str) -> [u8; 3] {
        reflectance_srgb8(&self.masstone_reflectance(id))
    }

    pub fn tint_srgb8(&self, id: &str, white_parts: f32) -> [u8; 3] {
        let pigment_fraction = 1.0 / (1.0 + white_parts.max(0.0));
        let white_fraction = 1.0 - pigment_fraction;
        let pigment = self.expect_pigment(id);
        let white = self.expect_pigment(TINT_WHITE_ID);
        let absorption = std::array::from_fn(|band| {
            pigment_fraction * pigment.absorption[band] + white_fraction * white.absorption[band]
        });
        let scattering = std::array::from_fn(|band| {
            pigment_fraction * pigment.scattering[band] + white_fraction * white.scattering[band]
        });
        reflectance_srgb8(&layer_over(
            &absorption,
            &scattering,
            MASSTONE_THICKNESS_UM,
            &BLACK_REFLECTANCE,
        ))
    }

    pub fn glaze_srgb8(&self, id: &str, over_black: bool) -> [u8; 3] {
        let pigment = self.expect_pigment(id);
        let below = if over_black {
            BLACK_REFLECTANCE
        } else {
            self.masstone_reflectance(TINT_WHITE_ID)
        };
        reflectance_srgb8(&layer_over(
            &pigment.absorption.map(|k| GLAZE_PIGMENT_FRACTION * k),
            &pigment.scattering.map(|s| GLAZE_PIGMENT_FRACTION * s),
            GLAZE_THICKNESS_UM,
            &below,
        ))
    }

    fn masstone_reflectance(&self, id: &str) -> BandValues {
        let pigment = self.expect_pigment(id);
        layer_over(
            &pigment.absorption,
            &pigment.scattering,
            MASSTONE_THICKNESS_UM,
            &BLACK_REFLECTANCE,
        )
    }

    fn expect_pigment(&self, id: &str) -> &Pigment {
        self.get(id)
            .unwrap_or_else(|| panic!("unknown pigment id {id:?}"))
    }
}

pub fn mixture_reflectance(
    db: &PigmentDb,
    slots: &[String],
    fractions: &[f32; MAX_SLOTS],
    thickness_um: f32,
    below: &BandValues,
) -> BandValues {
    let mut absorption = [0.0; BANDS];
    let mut scattering = [0.0; BANDS];
    for (slot, fraction) in slots.iter().zip(fractions) {
        let pigment = db.expect_pigment(slot);
        for band in 0..BANDS {
            absorption[band] += fraction * pigment.absorption[band];
            scattering[band] += fraction * pigment.scattering[band];
        }
    }
    layer_over(&absorption, &scattering, thickness_um, below)
}

pub fn mixture_srgb8(db: &PigmentDb, slots: &[String], fractions: &[f32; MAX_SLOTS]) -> [u8; 3] {
    reflectance_srgb8(&mixture_reflectance(
        db,
        slots,
        fractions,
        MASSTONE_THICKNESS_UM,
        &BLACK_REFLECTANCE,
    ))
}

fn layer_over(
    absorption: &BandValues,
    scattering: &BandValues,
    thickness_um: f32,
    below: &BandValues,
) -> BandValues {
    std::array::from_fn(|band| {
        composite_over(
            &km_layer(absorption[band], scattering[band], thickness_um),
            below[band],
        )
    })
}
