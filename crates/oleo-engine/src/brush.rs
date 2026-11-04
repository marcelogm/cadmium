use crate::pigments::{MAX_SLOTS, Mixture};
use crate::stroke::{BrushGeometry, PressureCurve, Tool};

pub const FULL_RESERVOIR_VOLUME_UM: f32 = 200.0;
pub const DEFAULT_MAX_DIAMETER_PX: f32 = 40.0;
pub const DEFAULT_MIN_DIAMETER_RATIO: f32 = 0.3;
pub const DEFAULT_SPACING: f32 = 0.05;
pub const DEFAULT_LOAD: f32 = 0.8;
pub const DEFAULT_MEDIUM: f32 = 0.1;
pub const MAX_MEDIUM: f32 = 0.95;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrushShape {
    Round,
    Flat,
    Fan,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushSettings {
    pub tool: Tool,
    pub shape: BrushShape,
    pub geometry: BrushGeometry,
    pub load: f32,
    pub medium: f32,
    pub recipe: Vec<(usize, f32)>,
}

impl Default for BrushSettings {
    fn default() -> Self {
        BrushSettings {
            tool: Tool::Brush,
            shape: BrushShape::Round,
            geometry: BrushGeometry {
                max_diameter_px: DEFAULT_MAX_DIAMETER_PX,
                min_diameter_ratio: DEFAULT_MIN_DIAMETER_RATIO,
                spacing: DEFAULT_SPACING,
                pressure_curve: PressureCurve::default(),
            },
            load: DEFAULT_LOAD,
            medium: DEFAULT_MEDIUM,
            recipe: vec![(0, 1.0)],
        }
    }
}

impl BrushSettings {
    pub fn reservoir_mixture(&self, slot_count: usize) -> Mixture {
        let usable_slots = slot_count.min(MAX_SLOTS);
        let mut parts = [0.0f32; MAX_SLOTS];
        for &(slot, amount) in &self.recipe {
            if slot < usable_slots {
                parts[slot] += amount.max(0.0);
            }
        }
        let total: f32 = parts.iter().sum();
        let pigment_share = 1.0 - self.medium.clamp(0.0, MAX_MEDIUM);
        let fractions = if total > 0.0 {
            parts.map(|part| part / total * pigment_share)
        } else {
            [0.0; MAX_SLOTS]
        };
        Mixture {
            volume_um: FULL_RESERVOIR_VOLUME_UM * self.load.clamp(0.0, 1.0),
            fractions,
        }
    }
}
