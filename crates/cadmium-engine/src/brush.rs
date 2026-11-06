use crate::pigments::{MAX_SLOTS, Mixture};
use crate::stroke::{BrushGeometry, PressureCurve, Tool};

mod file;
mod hair;
mod head;
mod library;

pub use file::{BRUSH_EXTENSION, BrushFileError, ReadBrush, brush_from_json, brush_to_json};
pub use hair::HairSpec;
pub use head::HeadSpec;
pub(crate) use head::{Section, SectionMix};
pub use library::{BrushSpec, default_brush, factory_brushes};

pub(crate) fn within(value: f32, [min, max]: [f32; 2], fallback: f32) -> f32 {
    if value.is_nan() {
        fallback
    } else {
        value.clamp(min, max)
    }
}

pub const FULL_RESERVOIR_VOLUME_UM: f32 = 200.0;
pub const DEFAULT_MAX_DIAMETER_PX: f32 = 40.0;
pub const DEFAULT_MIN_DIAMETER_RATIO: f32 = 0.3;
pub const DEFAULT_SPACING: f32 = 0.05;
pub const DEFAULT_LOAD: f32 = 0.8;
pub const DEFAULT_MEDIUM: f32 = 0.1;
pub const MAX_MEDIUM: f32 = 0.95;
pub const KNIFE_MAX_STEP_PX: f32 = 2.0;

#[derive(Clone, Debug, PartialEq)]
pub struct BrushSettings {
    pub tool: Tool,
    pub head: HeadSpec,
    pub hair: HairSpec,
    pub geometry: BrushGeometry,
    pub load: f32,
    pub medium: f32,
    pub recipe: Vec<(usize, f32)>,
}

impl Default for BrushSettings {
    fn default() -> Self {
        BrushSettings {
            tool: Tool::Brush,
            head: HeadSpec::ROUND,
            hair: HairSpec::default(),
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
    /// Takes up a brush: its head, its hair and its suggested size.
    pub fn wear(&mut self, brush: &BrushSpec) {
        self.head = brush.head;
        self.hair = brush.hair;
        self.geometry.max_diameter_px = brush.size_px;
    }

    pub fn stroke_geometry(&self) -> BrushGeometry {
        let profile = self.tool.profile();
        let mut geometry = full_diameter_spacing(self.geometry, profile.has_bristle_footprint());
        if profile.has_blade_footprint() && geometry.max_diameter_px > 0.0 {
            geometry.spacing = geometry
                .spacing
                .min(KNIFE_MAX_STEP_PX / geometry.max_diameter_px);
        }
        geometry
    }

    pub fn reservoir_mixture(&self, slot_count: usize) -> Mixture {
        let usable_slots = slot_count.min(MAX_SLOTS);
        let mut parts = [0.0f32; MAX_SLOTS];
        for &(slot, amount) in self.recipe.iter().filter(|(slot, _)| *slot < usable_slots) {
            parts[slot] += amount.max(0.0);
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

fn full_diameter_spacing(
    mut geometry: BrushGeometry,
    has_bristle_footprint: bool,
) -> BrushGeometry {
    if has_bristle_footprint {
        geometry.min_diameter_ratio = 1.0;
    }
    geometry
}

#[cfg(test)]
mod tests;
