mod specs;

use specs::SPECS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamGroup {
    Transfer,
    Dynamics,
    Tools,
    Footprint,
    Bristles,
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

const PARAM_COUNT: usize = 37;

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
