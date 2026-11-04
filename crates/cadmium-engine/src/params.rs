#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamGroup {
    Transfer,
    Dynamics,
    Tools,
    Footprint,
    Render,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub group: ParamGroup,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

const fn spec(
    key: &'static str,
    label: &'static str,
    group: ParamGroup,
    min: f32,
    max: f32,
    default: f32,
) -> ParamSpec {
    ParamSpec {
        key,
        label,
        group,
        min,
        max,
        default,
    }
}

const PARAM_COUNT: usize = 23;

const SPECS: [ParamSpec; PARAM_COUNT] = [
    spec(
        "transfer_fraction",
        "Fração de troca",
        ParamGroup::Transfer,
        0.0,
        1.0,
        0.1,
    ),
    spec(
        "velocity_cutoff",
        "Corte por velocidade",
        ParamGroup::Transfer,
        0.0,
        2.0,
        0.25,
    ),
    spec(
        "residual_um",
        "Resíduo mínimo (µm)",
        ParamGroup::Transfer,
        0.0,
        20.0,
        2.0,
    ),
    spec(
        "refill_rate",
        "Reabastecimento",
        ParamGroup::Transfer,
        0.0,
        1.0,
        0.03,
    ),
    spec(
        "reserve_factor",
        "Reserva",
        ParamGroup::Transfer,
        1.0,
        12.0,
        3.0,
    ),
    spec("advection", "Arrasto", ParamGroup::Dynamics, 0.0, 1.0, 0.5),
    spec("squeeze", "Espremer", ParamGroup::Dynamics, 0.0, 2.0, 0.6),
    spec(
        "penetration_um",
        "Profundidade do pincel (µm)",
        ParamGroup::Footprint,
        0.0,
        300.0,
        120.0,
    ),
    spec(
        "tilt_elongation",
        "Alongamento por inclinação",
        ParamGroup::Footprint,
        0.0,
        3.0,
        1.5,
    ),
    spec(
        "bristle_contrast",
        "Contraste das cerdas",
        ParamGroup::Footprint,
        0.0,
        1.0,
        0.7,
    ),
    spec(
        "bristle_count",
        "Número de cerdas",
        ParamGroup::Footprint,
        4.0,
        64.0,
        24.0,
    ),
    spec(
        "knife_gap_um",
        "Espessura da placa (µm)",
        ParamGroup::Tools,
        0.0,
        300.0,
        80.0,
    ),
    spec(
        "blend_diffusion",
        "Difusão (misturador)",
        ParamGroup::Tools,
        0.0,
        1.0,
        0.3,
    ),
    spec(
        "rag_strength",
        "Força do pano",
        ParamGroup::Tools,
        0.0,
        1.0,
        0.5,
    ),
    spec(
        "tube_flow_um_per_s",
        "Vazão do tubo (µm/s)",
        ParamGroup::Tools,
        20.0,
        2000.0,
        300.0,
    ),
    spec(
        "tube_cord_um",
        "Altura do cordão (µm)",
        ParamGroup::Tools,
        20.0,
        800.0,
        300.0,
    ),
    spec(
        "tube_max_um",
        "Altura máxima do tubo (µm)",
        ParamGroup::Tools,
        50.0,
        1500.0,
        500.0,
    ),
    spec(
        "draw_up_rate",
        "Carga pela bandeja (/s)",
        ParamGroup::Tools,
        0.1,
        10.0,
        2.0,
    ),
    spec(
        "impasto_scale",
        "Escala do relevo",
        ParamGroup::Render,
        0.0,
        5.0,
        2.0,
    ),
    spec(
        "light_azimuth_deg",
        "Luz: azimute",
        ParamGroup::Render,
        0.0,
        360.0,
        135.0,
    ),
    spec(
        "light_elevation_deg",
        "Luz: elevação",
        ParamGroup::Render,
        5.0,
        90.0,
        35.0,
    ),
    spec(
        "wet_gloss",
        "Brilho úmido",
        ParamGroup::Render,
        0.0,
        1.0,
        0.8,
    ),
    spec(
        "dry_roughness",
        "Rugosidade seca",
        ParamGroup::Render,
        0.05,
        1.0,
        0.6,
    ),
];

const UNKNOWN_KEY_VALUE: f32 = 0.0;

#[derive(Clone, Debug, PartialEq)]
pub struct SimParams {
    values: [f32; PARAM_COUNT],
}

impl Default for SimParams {
    fn default() -> Self {
        SimParams {
            values: SPECS.map(|spec| spec.default),
        }
    }
}

impl SimParams {
    pub fn specs() -> &'static [ParamSpec] {
        &SPECS
    }

    pub fn get(&self, key: &str) -> f32 {
        match index_of(key) {
            Some(index) => self.values[index],
            None => {
                debug_assert!(false, "unknown sim param {key}");
                UNKNOWN_KEY_VALUE
            }
        }
    }

    pub fn set(&mut self, key: &str, value: f32) {
        if value.is_nan() {
            return;
        }
        match index_of(key) {
            Some(index) => {
                let spec = &SPECS[index];
                self.values[index] = value.clamp(spec.min, spec.max);
            }
            None => debug_assert!(false, "unknown sim param {key}"),
        }
    }
}

fn index_of(key: &str) -> Option<usize> {
    SPECS.iter().position(|spec| spec.key == key)
}
