use cadmium_engine::brush::DEFAULT_MAX_DIAMETER_PX;
use cadmium_engine::engine::{TIP_MARGIN, TIP_SIZE};
use cadmium_engine::stroke::Tool;

const LARGE_DIAMETER_PX: f32 = 200.0;
const MAX_DIAMETER_PX: f32 = (TIP_SIZE - TIP_MARGIN) as f32;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Ground {
    Bare,
    Impasto,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Motion {
    Stroke,
    Settle,
    Shade,
    Present,
}

pub(crate) struct Scenario {
    pub(crate) name: &'static str,
    pub(crate) tool: Tool,
    pub(crate) diameter_px: f32,
    pub(crate) ground: Ground,
    pub(crate) motion: Motion,
}

pub(crate) const SCENARIOS: [Scenario; 10] = [
    scenario(
        "brush, default size",
        Tool::Brush,
        DEFAULT_MAX_DIAMETER_PX,
        Ground::Bare,
    ),
    scenario(
        "brush, 200 px",
        Tool::Brush,
        LARGE_DIAMETER_PX,
        Ground::Bare,
    ),
    scenario(
        "brush, max size",
        Tool::Brush,
        MAX_DIAMETER_PX,
        Ground::Bare,
    ),
    scenario(
        "brush, 200 px over impasto",
        Tool::Brush,
        LARGE_DIAMETER_PX,
        Ground::Impasto,
    ),
    scenario(
        "brush, max size over impasto",
        Tool::Brush,
        MAX_DIAMETER_PX,
        Ground::Impasto,
    ),
    scenario(
        "knife, default size over impasto",
        Tool::Knife,
        DEFAULT_MAX_DIAMETER_PX,
        Ground::Impasto,
    ),
    scenario(
        "blender, default size over impasto",
        Tool::Blender,
        DEFAULT_MAX_DIAMETER_PX,
        Ground::Impasto,
    ),
    Scenario {
        motion: Motion::Settle,
        ..scenario(
            "idle settle after a stroke",
            Tool::Brush,
            DEFAULT_MAX_DIAMETER_PX,
            Ground::Bare,
        )
    },
    Scenario {
        motion: Motion::Shade,
        ..scenario(
            "shade the full canvas",
            Tool::Brush,
            DEFAULT_MAX_DIAMETER_PX,
            Ground::Impasto,
        )
    },
    Scenario {
        motion: Motion::Present,
        ..scenario(
            "present the full canvas",
            Tool::Brush,
            DEFAULT_MAX_DIAMETER_PX,
            Ground::Impasto,
        )
    },
];

const fn scenario(name: &'static str, tool: Tool, diameter_px: f32, ground: Ground) -> Scenario {
    Scenario {
        name,
        tool,
        diameter_px,
        ground,
        motion: Motion::Stroke,
    }
}
