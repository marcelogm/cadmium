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

const PARAM_COUNT: usize = 23;

const SPECS: [ParamSpec; PARAM_COUNT] = [
    ParamSpec {
        key: "transfer_fraction",
        label: "Exchange Fraction",
        group: ParamGroup::Transfer,
        min: 0.0,
        max: 1.0,
        default: 0.1,
    },
    ParamSpec {
        key: "velocity_cutoff",
        label: "Velocity Cutoff",
        group: ParamGroup::Transfer,
        min: 0.0,
        max: 2.0,
        default: 0.25,
    },
    ParamSpec {
        key: "residual_um",
        label: "Minimum Residue (µm)",
        group: ParamGroup::Transfer,
        min: 0.0,
        max: 20.0,
        default: 2.0,
    },
    ParamSpec {
        key: "refill_rate",
        label: "Refill Rate",
        group: ParamGroup::Transfer,
        min: 0.0,
        max: 1.0,
        default: 0.03,
    },
    ParamSpec {
        key: "reserve_factor",
        label: "Reserve",
        group: ParamGroup::Transfer,
        min: 1.0,
        max: 12.0,
        default: 3.0,
    },
    ParamSpec {
        key: "advection",
        label: "Drag",
        group: ParamGroup::Dynamics,
        min: 0.0,
        max: 1.0,
        default: 0.5,
    },
    ParamSpec {
        key: "squeeze",
        label: "Squeeze",
        group: ParamGroup::Dynamics,
        min: 0.0,
        max: 2.0,
        default: 0.6,
    },
    ParamSpec {
        key: "penetration_um",
        label: "Brush Depth (µm)",
        group: ParamGroup::Footprint,
        min: 0.0,
        max: 300.0,
        default: 120.0,
    },
    ParamSpec {
        key: "tilt_elongation",
        label: "Tilt Elongation",
        group: ParamGroup::Footprint,
        min: 0.0,
        max: 3.0,
        default: 1.5,
    },
    ParamSpec {
        key: "bristle_contrast",
        label: "Bristle Contrast",
        group: ParamGroup::Footprint,
        min: 0.0,
        max: 1.0,
        default: 0.7,
    },
    ParamSpec {
        key: "bristle_count",
        label: "Bristle Count",
        group: ParamGroup::Footprint,
        min: 4.0,
        max: 64.0,
        default: 24.0,
    },
    ParamSpec {
        key: "knife_gap_um",
        label: "Slab Thickness (µm)",
        group: ParamGroup::Tools,
        min: 0.0,
        max: 300.0,
        default: 80.0,
    },
    ParamSpec {
        key: "blend_diffusion",
        label: "Diffusion (Blender)",
        group: ParamGroup::Tools,
        min: 0.0,
        max: 1.0,
        default: 0.3,
    },
    ParamSpec {
        key: "rag_strength",
        label: "Rag Strength",
        group: ParamGroup::Tools,
        min: 0.0,
        max: 1.0,
        default: 0.5,
    },
    ParamSpec {
        key: "tube_flow_um_per_s",
        label: "Blob Growth (µm/s)",
        group: ParamGroup::Tools,
        min: 20.0,
        max: 2000.0,
        default: 300.0,
    },
    ParamSpec {
        key: "tube_cord_um",
        label: "Bead Height (µm)",
        group: ParamGroup::Tools,
        min: 20.0,
        max: 500.0,
        default: 300.0,
    },
    ParamSpec {
        key: "tube_max_um",
        label: "Maximum Tube Height (µm)",
        group: ParamGroup::Tools,
        min: 50.0,
        max: 1500.0,
        default: 500.0,
    },
    ParamSpec {
        key: "draw_up_rate",
        label: "Tray Loading Rate (/s)",
        group: ParamGroup::Tools,
        min: 0.1,
        max: 10.0,
        default: 2.0,
    },
    ParamSpec {
        key: "impasto_scale",
        label: "Relief Scale",
        group: ParamGroup::Render,
        min: 0.0,
        max: 5.0,
        default: 2.0,
    },
    ParamSpec {
        key: "light_azimuth_deg",
        label: "Light Azimuth",
        group: ParamGroup::Render,
        min: 0.0,
        max: 360.0,
        default: 135.0,
    },
    ParamSpec {
        key: "light_elevation_deg",
        label: "Light Elevation",
        group: ParamGroup::Render,
        min: 5.0,
        max: 90.0,
        default: 35.0,
    },
    ParamSpec {
        key: "wet_gloss",
        label: "Wet Gloss",
        group: ParamGroup::Render,
        min: 0.0,
        max: 1.0,
        default: 0.8,
    },
    ParamSpec {
        key: "dry_roughness",
        label: "Dry Roughness",
        group: ParamGroup::Render,
        min: 0.05,
        max: 1.0,
        default: 0.6,
    },
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
