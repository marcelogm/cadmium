use crate::params::SimParams;
use crate::stroke::Tool;

mod blender;
mod brush;
mod eyedropper;
mod knife;
mod rag;

const TOOL_MODE_BRUSH: u32 = 0;
pub(crate) const TOOL_MODE_KNIFE: u32 = 1;
const TOOL_MODE_BLENDER: u32 = 2;
const TOOL_MODE_RAG: u32 = 3;
const TOOL_MODE_EYEDROPPER: u32 = 4;
const CONTACT_REST_FOLLOW: f32 = 0.25;

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FootprintShape {
    Ellipse,
    Blade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DabStage {
    Exchange,
    Apply,
    Capillary,
    Refill,
    Spread,
    Blade,
    Wipe,
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

    pub(crate) fn places_dabs(&self) -> bool {
        self.footprint.is_some()
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
}

impl Spreading {
    pub(crate) fn rates(self, params: &SimParams) -> SpreadingRates {
        let advection = params.get("advection");
        let squeeze = params.get("squeeze");
        match self {
            Spreading::Drag(drag) => SpreadingRates {
                advection: drag,
                squeeze: 0.0,
                diffusion: 0.0,
            },
            Spreading::AdvectAndDiffuse => SpreadingRates {
                advection,
                squeeze,
                diffusion: params.get("blend_diffusion"),
            },
            Spreading::Advect => SpreadingRates {
                advection,
                squeeze,
                diffusion: 0.0,
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
            Tool::Eyedropper => &eyedropper::EYEDROPPER,
        }
    }
}

#[cfg(test)]
mod tests;
