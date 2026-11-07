use crate::params::SimParams;
use crate::stroke::Tool;

mod blender;
mod brush;
mod eyedropper;
mod knife;
mod mixing_knife;
mod rag;

const TOOL_MODE_BRUSH: u32 = 0;
pub(crate) const TOOL_MODE_KNIFE: u32 = 1;
const TOOL_MODE_BLENDER: u32 = 2;
const TOOL_MODE_RAG: u32 = 3;
const TOOL_MODE_EYEDROPPER: u32 = 4;
pub(crate) const TOOL_MODE_MIXING_KNIFE: u32 = 5;
const CONTACT_REST_FOLLOW: f32 = 0.25;
/// The shader modes whose dabs are a flat rectangular blade rather than a tuft;
/// the WGSL `is_blade_mode` is generated from this list.
pub(crate) const BLADE_MODES: [u32; 2] = [TOOL_MODE_KNIFE, TOOL_MODE_MIXING_KNIFE];

#[derive(Debug)]
pub struct ToolProfile {
    pub(crate) shader_mode: u32,
    pub(crate) spreading: Spreading,
    pub holder: Option<Holder>,
    pub(crate) footprint: Option<FootprintShape>,
    pub(crate) rest_follow: f32,
    pub(crate) dab_stages: &'static [DabStage],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    Reservoir,
    Blade,
    Face,
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
    Face,
    Wipe,
}

impl ToolProfile {
    pub fn holds_on_blade(&self) -> bool {
        self.holder == Some(Holder::Blade)
    }

    pub fn holds_on_face(&self) -> bool {
        self.holder == Some(Holder::Face)
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

    pub(crate) fn wipes(&self) -> bool {
        self.dab_stages.contains(&DabStage::Wipe)
    }
}

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
            Tool::MixingKnife => &mixing_knife::MIXING_KNIFE,
            Tool::Blender => &blender::BLENDER,
            Tool::Rag => &rag::RAG,
            Tool::Eyedropper => &eyedropper::EYEDROPPER,
        }
    }
}

#[cfg(test)]
mod tests;
