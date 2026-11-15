use crate::params::SimParams;
use crate::stroke::Tool;

mod blender;
mod brush;
mod eyedropper;
mod knife;
mod pencil;
mod rag;

const TOOL_MODE_BRUSH: u32 = 0;
pub(crate) const TOOL_MODE_KNIFE: u32 = 1;
const TOOL_MODE_BLENDER: u32 = 2;
const TOOL_MODE_RAG: u32 = 3;
const TOOL_MODE_EYEDROPPER: u32 = 4;
const TOOL_MODE_PENCIL: u32 = 6;
const CONTACT_REST_FOLLOW: f32 = 0.25;
/// The shader modes whose dabs are a flat rectangular blade rather than a tuft;
/// the WGSL `is_blade_mode` is generated from this list.
pub(crate) const BLADE_MODES: [u32; 1] = [TOOL_MODE_KNIFE];
const RAG_STRENGTH_PARAM: &str = "rag_strength";
const PENCIL_STRENGTH_PARAM: &str = "pencil_strength";

#[derive(Debug)]
pub struct ToolProfile {
    pub(crate) shader_mode: u32,
    pub(crate) spreading: Spreading,
    pub holder: Option<Holder>,
    pub(crate) footprint: Option<FootprintShape>,
    pub(crate) rest_follow: f32,
    pub(crate) dab_stages: &'static [DabStage],
    /// Param read into the dab's tool strength; only the rag and pencil shaders use it.
    pub(crate) strength_param: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    Reservoir,
    Blade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FootprintShape {
    Ellipse,
    Blade,
    Bristles,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DabStage {
    Bristles,
    Contact,
    Exchange,
    Apply,
    Capillary,
    Refill,
    Spread,
    Blade,
    Wipe,
    Graphite,
}

impl ToolProfile {
    pub fn holds_on_blade(&self) -> bool {
        self.holder == Some(Holder::Blade)
    }

    pub(crate) fn has_blade_footprint(&self) -> bool {
        self.footprint == Some(FootprintShape::Blade)
    }

    pub(crate) fn has_ellipse_footprint(&self) -> bool {
        self.footprint == Some(FootprintShape::Ellipse)
    }

    pub(crate) fn has_bristle_footprint(&self) -> bool {
        self.footprint == Some(FootprintShape::Bristles)
    }

    pub(crate) fn places_dabs(&self) -> bool {
        self.footprint.is_some()
    }

    pub(crate) fn moves_bristles(&self) -> bool {
        self.dab_stages.contains(&DabStage::Bristles)
    }

    /// Whether any of its dabs reach the wet paint; graphite alone leaves it be.
    pub(crate) fn touches_paint(&self) -> bool {
        self.dab_stages
            .iter()
            .any(|stage| !PAINTLESS_STAGES.contains(stage))
    }

    /// Whether its dabs move paint between the surface and what the tool holds.
    pub(crate) fn changes_held_paint(&self) -> bool {
        self.dab_stages
            .iter()
            .any(|stage| matches!(stage, DabStage::Exchange | DabStage::Blade))
    }

    /// Whether its dabs settle on the surface: only the exchange and blade stages read the rest.
    pub(crate) fn rests_on_surface(&self) -> bool {
        self.dab_stages
            .iter()
            .any(|stage| matches!(stage, DabStage::Exchange | DabStage::Blade))
    }
}

const PAINTLESS_STAGES: [DabStage; 1] = [DabStage::Graphite];

#[derive(Clone, Copy, Debug)]
pub(crate) enum Spreading {
    Advect,
    Drag(f32),
    AdvectAndDiffuse,
}

pub(crate) struct SpreadingRates {
    pub(crate) advection: f32,
    pub(crate) squeeze: f32,
    pub(crate) diffusion: f32,
    pub(crate) slump: f32,
}

impl Spreading {
    pub(crate) fn rates(self, params: &SimParams) -> SpreadingRates {
        let advection = params.get("advection");
        let squeeze = params.get("squeeze");
        let slump = params.get("slump_rate");
        match self {
            Spreading::Drag(drag) => SpreadingRates {
                advection: drag,
                squeeze: 0.0,
                diffusion: 0.0,
                slump: 0.0,
            },
            Spreading::AdvectAndDiffuse => SpreadingRates {
                advection,
                squeeze,
                diffusion: params.get("blend_diffusion"),
                slump,
            },
            Spreading::Advect => SpreadingRates {
                advection,
                squeeze,
                diffusion: 0.0,
                slump,
            },
        }
    }
}

impl Tool {
    pub fn profile(self) -> &'static ToolProfile {
        match self {
            Tool::Brush => &brush::BRUSH,
            Tool::Knife => &knife::KNIFE,
            Tool::Blender => &blender::BLENDER,
            Tool::Rag => &rag::RAG,
            Tool::Pencil => &pencil::PENCIL,
            Tool::Eyedropper => &eyedropper::EYEDROPPER,
        }
    }
}

#[cfg(test)]
mod tests;
